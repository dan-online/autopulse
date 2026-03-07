use futures::future::BoxFuture;
use futures::FutureExt;
use reqwest::header::HeaderMap;
use serde::Serialize;
use std::sync::OnceLock;
use std::time::Duration;
use tracing::trace;

pub struct WebhookResponse {
    pub headers: HeaderMap,
    pub body: String,
    pub success: bool,
    pub status_code: Option<u16>,
}

pub struct ReqwestWebhookClient {
    client: reqwest::Client,
}

impl ReqwestWebhookClient {
    pub fn new(timeout: Duration) -> anyhow::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| anyhow::anyhow!("failed to build reqwest client: {e}"))?;

        Ok(Self { client })
    }
}

pub trait WebhookHttpClient: Send + Sync {
    fn post_json<'a>(
        &'a self,
        url: &'a str,
        payload: &'a serde_json::Value,
    ) -> BoxFuture<'a, anyhow::Result<WebhookResponse>>;
}

pub trait WebhookSleeper: Send + Sync {
    fn sleep<'a>(&'a self, duration: Duration) -> BoxFuture<'a, ()>;
}

pub trait WebhookClock: Send + Sync {
    fn now(&self) -> u64;
}

pub struct TokioWebhookSleeper;

pub struct UtcWebhookClock;

pub struct WebhookSender<C, S, N> {
    client: C,
    sleeper: S,
    clock: N,
}

impl<C, S, N> WebhookSender<C, S, N>
where
    C: WebhookHttpClient,
    S: WebhookSleeper,
    N: WebhookClock,
{
    fn serialize_messages<T>(messages: &[T]) -> anyhow::Result<Vec<serde_json::Value>>
    where
        T: Serialize,
    {
        messages
            .iter()
            .map(serde_json::to_value)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub const fn new(client: C, sleeper: S, clock: N) -> Self {
        Self {
            client,
            sleeper,
            clock,
        }
    }

    pub fn send_json<'a, T>(
        &'a self,
        url: &'a str,
        messages: &'a [T],
        retries: u8,
    ) -> BoxFuture<'a, anyhow::Result<()>>
    where
        T: Serialize,
    {
        let messages = Self::serialize_messages(messages);

        async move {
            let messages = messages?;
            let max_retries = retries;
            let mut retries = retries;
            let mut index = 0;

            while index < messages.len() {
                let message = &messages[index];

                let response = match self.client.post_json(url, message).await {
                    Ok(resp) => resp,
                    Err(err) => {
                        if retries == 0 {
                            return Err(err);
                        }

                        let backoff = Duration::from_secs(2u64.pow((max_retries - retries) as u32));
                        trace!(
                            "network error, retrying in {} seconds: {err}",
                            backoff.as_secs()
                        );
                        self.sleeper.sleep(backoff).await;
                        retries -= 1;
                        continue;
                    }
                };

                if response.success {
                    index += 1;
                    retries = max_retries;
                    continue;
                }

                if let Some(wait) = self.retry_delay(&response.headers) {
                    if retries == 0 {
                        return Err(anyhow::anyhow!(
                            "failed to send webhook, retries exhausted: {}",
                            response.body
                        ));
                    }

                    trace!("rate limited, waiting for {} seconds", wait.as_secs());

                    self.sleeper.sleep(wait).await;
                    retries -= 1;
                    continue;
                }

                let is_server_error = response
                    .status_code
                    .is_some_and(|code| (500..600).contains(&code));

                if is_server_error && retries > 0 {
                    let backoff = Duration::from_secs(2u64.pow((max_retries - retries) as u32));
                    trace!(
                        "server error {}, retrying in {} seconds",
                        response.status_code.unwrap_or(0),
                        backoff.as_secs()
                    );
                    self.sleeper.sleep(backoff).await;
                    retries -= 1;
                    continue;
                }

                return Err(anyhow::anyhow!("failed to send webhook: {}", response.body));
            }

            Ok(())
        }
        .boxed()
    }

    fn retry_delay(&self, headers: &HeaderMap) -> Option<Duration> {
        if let Some(retry_after) = headers.get("Retry-After") {
            let retry_after = retry_after.to_str().ok()?;
            let retry_after = retry_after.parse::<u64>().ok()?;

            return Some(Duration::from_secs(retry_after));
        }

        if let Some(reset) = headers.get("X-RateLimit-Reset") {
            let reset = reset.to_str().ok()?;
            let reset = reset.parse::<u64>().ok()?;
            let now = self.clock.now();

            if reset > now {
                return Some(Duration::from_secs(reset - now));
            }
        }

        None
    }
}

impl WebhookHttpClient for ReqwestWebhookClient {
    fn post_json<'a>(
        &'a self,
        url: &'a str,
        payload: &'a serde_json::Value,
    ) -> BoxFuture<'a, anyhow::Result<WebhookResponse>> {
        async move {
            let response = self
                .client
                .post(url)
                .json(&payload)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            let headers = response.headers().clone();
            let status_code = response.status().as_u16();
            let success = response.status().is_success();
            let body = if success {
                String::new()
            } else {
                response
                    .text()
                    .await
                    .unwrap_or_else(|_| "no body".to_string())
            };

            Ok(WebhookResponse {
                headers,
                body,
                success,
                status_code: Some(status_code),
            })
        }
        .boxed()
    }
}

impl WebhookSleeper for TokioWebhookSleeper {
    fn sleep<'a>(&'a self, duration: Duration) -> BoxFuture<'a, ()> {
        async move {
            tokio::time::sleep(duration).await;
        }
        .boxed()
    }
}

impl WebhookClock for UtcWebhookClock {
    fn now(&self) -> u64 {
        chrono::Utc::now().timestamp() as u64
    }
}

type SharedSender = WebhookSender<ReqwestWebhookClient, TokioWebhookSleeper, UtcWebhookClock>;

static SHARED_SENDER: OnceLock<Result<SharedSender, String>> = OnceLock::new();

/// Returns a reference to the shared webhook sender, initialising it on first call.
///
/// The `timeout` parameter is only used during the first initialisation.
/// Subsequent calls return the same sender regardless of the timeout value passed.
pub fn shared_sender(timeout: Duration) -> anyhow::Result<&'static SharedSender> {
    SHARED_SENDER
        .get_or_init(|| {
            ReqwestWebhookClient::new(timeout)
                .map(|client| WebhookSender::new(client, TokioWebhookSleeper, UtcWebhookClock))
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(|e| anyhow::anyhow!("{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt;
    use reqwest::header::{HeaderValue, CONTENT_TYPE};
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    enum FakeResponse {
        Ok(WebhookResponse),
        Err(String),
    }

    #[derive(Clone)]
    struct FakeClient {
        responses: Arc<Mutex<VecDeque<FakeResponse>>>,
        requests: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    impl FakeClient {
        fn new(responses: Vec<WebhookResponse>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(
                    responses.into_iter().map(FakeResponse::Ok).collect(),
                )),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn with_results(responses: Vec<FakeResponse>) -> Self {
            Self {
                responses: Arc::new(Mutex::new(responses.into())),
                requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn requests(&self) -> Vec<serde_json::Value> {
            self.requests.lock().unwrap().clone()
        }
    }

    impl WebhookHttpClient for FakeClient {
        fn post_json<'a>(
            &'a self,
            _url: &'a str,
            payload: &'a serde_json::Value,
        ) -> BoxFuture<'a, anyhow::Result<WebhookResponse>> {
            async move {
                self.requests.lock().unwrap().push(payload.clone());
                match self
                    .responses
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("missing fake response")
                {
                    FakeResponse::Ok(resp) => Ok(resp),
                    FakeResponse::Err(msg) => Err(anyhow::anyhow!(msg)),
                }
            }
            .boxed()
        }
    }

    #[derive(Default)]
    struct FakeSleeper {
        sleeps: Mutex<Vec<Duration>>,
    }

    impl FakeSleeper {
        fn sleeps(&self) -> Vec<Duration> {
            self.sleeps.lock().unwrap().clone()
        }
    }

    impl WebhookSleeper for FakeSleeper {
        fn sleep<'a>(&'a self, duration: Duration) -> BoxFuture<'a, ()> {
            async move {
                self.sleeps.lock().unwrap().push(duration);
            }
            .boxed()
        }
    }

    struct FakeClock {
        now: u64,
    }

    impl WebhookClock for FakeClock {
        fn now(&self) -> u64 {
            self.now
        }
    }

    fn success_response() -> WebhookResponse {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        WebhookResponse {
            headers,
            body: String::new(),
            success: true,
            status_code: Some(200),
        }
    }

    fn rate_limited_response(reset: u64) -> WebhookResponse {
        let mut headers = HeaderMap::new();
        headers.insert(
            "X-RateLimit-Reset",
            HeaderValue::from_str(&reset.to_string()).unwrap(),
        );

        WebhookResponse {
            headers,
            body: "rate limited".to_string(),
            success: false,
            status_code: Some(429),
        }
    }

    fn retry_after_response(retry_after: u64) -> WebhookResponse {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Retry-After",
            HeaderValue::from_str(&retry_after.to_string()).unwrap(),
        );

        WebhookResponse {
            headers,
            body: "rate limited".to_string(),
            success: false,
            status_code: Some(429),
        }
    }

    fn server_error_response() -> WebhookResponse {
        WebhookResponse {
            headers: HeaderMap::new(),
            body: "Internal Server Error".to_string(),
            success: false,
            status_code: Some(500),
        }
    }

    #[tokio::test]
    async fn send_json_retries_only_unsent_messages_after_rate_limit_reset() {
        let client = FakeClient::new(vec![
            success_response(),
            rate_limited_response(105),
            success_response(),
        ]);
        let sleeper = FakeSleeper::default();
        let sender = WebhookSender::new(client.clone(), sleeper, FakeClock { now: 100 });
        let messages = [
            serde_json::json!({ "id": 1 }),
            serde_json::json!({ "id": 2 }),
        ];

        sender
            .send_json("https://example.com/webhook", &messages, 1)
            .await
            .unwrap();

        assert_eq!(
            client.requests(),
            vec![
                messages[0].clone(),
                messages[1].clone(),
                messages[1].clone()
            ]
        );
        assert_eq!(sender.sleeper.sleeps(), vec![Duration::from_secs(5)]);
    }

    #[tokio::test]
    async fn send_json_retries_after_standard_retry_after_header() {
        let client = FakeClient::new(vec![retry_after_response(3), success_response()]);
        let sleeper = FakeSleeper::default();
        let sender = WebhookSender::new(client.clone(), sleeper, FakeClock { now: 100 });
        let messages = [serde_json::json!({ "id": 1 })];

        sender
            .send_json("https://example.com/webhook", &messages, 1)
            .await
            .unwrap();

        assert_eq!(
            client.requests(),
            vec![messages[0].clone(), messages[0].clone()]
        );
        assert_eq!(sender.sleeper.sleeps(), vec![Duration::from_secs(3)]);
    }

    #[tokio::test]
    async fn send_json_returns_retries_exhausted_when_rate_limit_persists() {
        let client = FakeClient::new(vec![rate_limited_response(105)]);
        let sender = WebhookSender::new(client, FakeSleeper::default(), FakeClock { now: 100 });
        let messages = [serde_json::json!({ "id": 1 })];

        let error = sender
            .send_json("https://example.com/webhook", &messages, 0)
            .await
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "failed to send webhook, retries exhausted: rate limited"
        );
    }

    #[tokio::test]
    async fn send_json_retries_on_5xx_with_exponential_backoff() {
        let client = FakeClient::new(vec![server_error_response(), success_response()]);
        let sleeper = FakeSleeper::default();
        let sender = WebhookSender::new(client.clone(), sleeper, FakeClock { now: 100 });
        let messages = [serde_json::json!({ "id": 1 })];

        sender
            .send_json("https://example.com/webhook", &messages, 2)
            .await
            .unwrap();

        assert_eq!(
            client.requests(),
            vec![messages[0].clone(), messages[0].clone()]
        );
        // max_retries=2, retries=2 on first failure: backoff = 2^(2-2) = 1s
        assert_eq!(sender.sleeper.sleeps(), vec![Duration::from_secs(1)]);
    }

    #[tokio::test]
    async fn send_json_retries_on_network_error_with_exponential_backoff() {
        let client = FakeClient::with_results(vec![
            FakeResponse::Err("connection reset".to_string()),
            FakeResponse::Ok(success_response()),
        ]);
        let sleeper = FakeSleeper::default();
        let sender = WebhookSender::new(client.clone(), sleeper, FakeClock { now: 100 });
        let messages = [serde_json::json!({ "id": 1 })];

        sender
            .send_json("https://example.com/webhook", &messages, 2)
            .await
            .unwrap();

        assert_eq!(
            client.requests(),
            vec![messages[0].clone(), messages[0].clone()]
        );
        // max_retries=2, retries=2 on first failure: backoff = 2^(2-2) = 1s
        assert_eq!(sender.sleeper.sleeps(), vec![Duration::from_secs(1)]);
    }

    #[tokio::test]
    async fn send_json_gives_up_after_retries_exhausted_on_5xx() {
        let client = FakeClient::new(vec![server_error_response(), server_error_response()]);
        let sleeper = FakeSleeper::default();
        let sender = WebhookSender::new(client, sleeper, FakeClock { now: 100 });
        let messages = [serde_json::json!({ "id": 1 })];

        let error = sender
            .send_json("https://example.com/webhook", &messages, 1)
            .await
            .unwrap_err();

        assert_eq!(
            error.to_string(),
            "failed to send webhook: Internal Server Error"
        );
        // max_retries=1, retries=1 on first failure: backoff = 2^(1-1) = 1s
        assert_eq!(sender.sleeper.sleeps(), vec![Duration::from_secs(1)]);
    }
}
