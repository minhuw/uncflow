//! Generic register abstractions for type-safe MSR programming

/// Trait for register layouts that can be converted to/from raw MSR values
///
/// This trait provides type-safe conversion between structured register
/// layouts and the raw 64-bit values that are written to/read from MSRs.
///
/// # Example
///
/// ```ignore
/// use uncflow_raw::register::RegisterLayout;
///
/// #[derive(Debug, Default)]
/// struct MyControl {
///     enable: bool,
///     threshold: u8,
/// }
///
/// impl RegisterLayout for MyControl {
///     fn to_msr_value(&self) -> u64 {
///         (if self.enable { 1 } else { 0 })
///             | ((self.threshold as u64) << 8)
///     }
///
///     fn from_msr_value(value: u64) -> Self {
///         Self {
///             enable: (value & 1) != 0,
///             threshold: ((value >> 8) & 0xFF) as u8,
///         }
///     }
/// }
/// ```
pub trait RegisterLayout: Sized {
    /// Convert this register layout to a raw MSR value
    fn to_msr_value(&self) -> u64;

    /// Parse a raw MSR value into this register layout
    fn from_msr_value(value: u64) -> Self;

    /// Validate that the register values are within acceptable ranges
    ///
    /// Returns `Ok(())` if valid, or an error message if invalid.
    fn validate(&self) -> Result<(), &'static str> {
        Ok(())
    }
}

/// A hardware register with address and typed layout
///
/// This struct combines an MSR address with a typed register layout,
/// providing a convenient abstraction for working with specific registers.
///
/// # Example
///
/// ```ignore
/// use uncflow_raw::register::Register;
///
/// let reg = Register::new(0xE01, MyControl {
///     enable: true,
///     threshold: 10,
/// });
///
/// // Get MSR value to write
/// let value = reg.layout.to_msr_value();
/// ```
#[derive(Debug, Clone, Copy)]
pub struct Register<T: RegisterLayout> {
    /// MSR address
    pub address: u64,
    /// Typed register layout
    pub layout: T,
}

impl<T: RegisterLayout> Register<T> {
    /// Create a new register with the given address and layout
    pub fn new(address: u64, layout: T) -> Self {
        Self { address, layout }
    }

    /// Create a register with default layout
    pub fn with_address(address: u64) -> Self
    where
        T: Default,
    {
        Self {
            address,
            layout: T::default(),
        }
    }

    /// Validate the register layout
    pub fn validate(&self) -> Result<(), &'static str> {
        self.layout.validate()
    }

    /// Get the MSR value for this register
    pub fn to_msr_value(&self) -> u64 {
        self.layout.to_msr_value()
    }

    /// Update the layout from an MSR value
    pub fn from_msr_value(&mut self, value: u64) {
        self.layout = T::from_msr_value(value);
    }
}
