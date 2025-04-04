#[doc(hidden)]
pub mod manager;
#[doc(hidden)]
pub mod runner;

/// Settings for the service
pub mod settings;

#[cfg(test)]
mod tests {
    mod triggers;
}
