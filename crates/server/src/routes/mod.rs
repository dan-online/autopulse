/// GET &nbsp; - `/`
///
/// Retrieves the current version of the application.
///
/// # Responses
///
/// The response will be a JSON object with the following field:
///
/// - `autopulse`: The version number of the application, prefixed with "v". For example, `"v1.2.3"`.
///
/// # Example Response
///
/// ```json
/// {
///   "autopulse": "v1.2.3"
/// }
/// ```
pub mod index;

/// GET &nbsp; - `/status/{id}`
///
/// Retrieves the file event from the database with the specified ID.
///
/// Note: Requires authentication.
///
/// # Responses
///
/// - **200 OK**: Returns the file event with the specified ID.
/// - **401 Unauthorized**: Returned if the request is not authenticated.
/// - **404 Not Found**: Returned if the file event with the specified ID does not exist.
pub mod status;

/// GET &nbsp; - `/stats`
///
/// Retrieve the current service statistics and measures the database response time.
///
/// # Responses
///
/// - **200 OK**: Returns a [`StatsResponse`](stats::StatsResponse) object containing the service statistics and response time.
pub mod stats;

/// GET &nbsp; - `/list?status={status}&page={page}&limit={limit}&sort={sort}`
///
/// Returns a list of scan events from the database.
///
/// # Query Parameters
///
/// - `status`: Filter the scan events by process status. Can be one of `pending`, `complete`, `retry`, or `failed`.
/// - `page`: The page number to retrieve.
/// - `limit`: The number of items to retrieve per page.
/// - `sort`: The field to sort the results by. Can be one of `id`, `file_path`, `process_status`, `event_source`, `created_at`, or `updated_at`.
///
/// See [`list::ListQuery`] for more information.
///
/// # Responses
///
/// - **200 OK**: Returns a list of [`ScanEvent`](autopulse_database::models::ScanEvent) objects.
/// - **401 Unauthorized**: Returned if the request is not authenticated.
pub mod list;

/// POST - `/login`
///
/// Authenticates the user with the provided credentials and returns ok if successful.
///
/// # Responses
///
/// - **200 OK**: Returns a JSON object with the field `status` set to `"ok"`.
/// - **401 Unauthorized**: Returned if the request is not authenticated.
pub mod login;

/// GET/POST - `/trigger/{name}`
///
/// Triggers a new scan event. Where name is as defined in the settings file.
///
/// GET is used for manual triggers, while POST is used for automated triggers.
///
/// See the [Triggers](autopulse_service::settings::triggers) module for more information.
///
/// # Responses
///
/// - **200 OK**: Returns a [`ScanEvent`](autopulse_database::models::ScanEvent) object containing the scan event.
/// - **401 Unauthorized**: Returned if the request is not authenticated.
/// - **404 Not Found**: Returned if the trigger does not exist.
/// - **400 Bad Request**: Returned if the request is invalid.
///
/// # Example:
///
/// ```yml
/// triggers:
///  my_sonarr:
///   type: sonarr
///
///  my_manual:
///   type: manual
/// ```
///
/// ```bash
/// $ curl -u 'admin:password' 'http://localhost:8080/trigger/my_manual?path=/path/to/file'
/// {
///     ...
/// }
/// ```
pub mod triggers;
