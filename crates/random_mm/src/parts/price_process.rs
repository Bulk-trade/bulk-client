use rand_distr::{Distribution, Normal};

/// Ornstein-Uhlenbeck process for mid-price simulation.
///
/// `dX = κ(μ − X) dt + σ dW`
pub struct OuProcess {
    p0: f64,
    kappa: f64,
    sigma: f64,
    mu: f64,
    cumr: f64,
    price: f64,
}

impl OuProcess {
    /// Create OU process
    ///
    /// # Arguments
    /// - `p0`: price basis
    /// - `kappa`: mean-reversion parameter
    /// - `sigma`: volatility
    pub(crate) fn new(p0: f64, kappa: f64, sigma: f64) -> Self {
        Self {
            p0,
            kappa,
            sigma,
            mu: 0.0,
            cumr: 0.0,
            price: p0,
        }
    }

    /// Advance the process by `dt` seconds and return the new price.
    ///
    /// # Arguments
    /// - `dt`: time delta
    pub(crate) fn step(&mut self, dt: f64) -> f64 {
        let mut rng = rand::rng();
        let normal = Normal::new(0.0, dt.sqrt()).unwrap();
        let dw: f64 = normal.sample(&mut rng);

        self.cumr += self.kappa * (self.mu - self.cumr) * dt + self.sigma * dw;
        self.price = self.p0 * (self.cumr * 1e-4).exp();
        self.price
    }
}