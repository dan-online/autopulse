/// GET `/`
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

/// GET `/status/{id}`
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

/// GET `/stats`
///
/// This asynchronous function is triggered when a GET request is made to the `/stats` endpoint.
/// It retrieves the current service statistics and measures the response time.
///
/// # Responses
///
/// - **200 OK**: Returns a [StatsResponse](stats::StatsResponse) object containing the service statistics and response time.
/// - **500 Internal Server Error**: Returned if there is an issue retrieving the statistics.
pub mod stats;

pub mod list;
/// GET/POST `/trigger/{name}`
///
/// Triggers a new scan event. Where name is as defined in the settings file.
///
/// GET is used for manual triggers, while POST is used for automated triggers.
///
/// See the [Triggers](crate::service::triggers) module for more information.
///
/// # Responses
///
/// - **200 OK**: Returns a [ScanEvent](crate::db::models::ScanEvent) object containing the scan event.
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
pub mod ui;
