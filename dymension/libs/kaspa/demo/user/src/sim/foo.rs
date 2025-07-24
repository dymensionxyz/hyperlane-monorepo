pub struct TrafficSim {
    n: u32,
}

impl TrafficSim {
    pub fn new(n: u32) -> Self {
        Self { n }
    }

    pub async fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}