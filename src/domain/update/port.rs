/// Boundary for installing or rolling back firmware images.
pub trait UpdatePort {
    type Error;

    /// Installs the latest available firmware image and restarts if an image was installed.
    async fn update(&mut self) -> Result<(), Self::Error>;

    /// Marks the currently running pending image as valid.
    ///
    /// Returns `true` when an image was confirmed.
    fn confirm(&mut self) -> Result<bool, Self::Error>;
}
