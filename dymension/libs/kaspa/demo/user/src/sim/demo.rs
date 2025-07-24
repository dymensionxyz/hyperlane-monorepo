use super::foo::Params;
use super::util::{as_kas, SOMPI_PER_KAS};
use std::time::Duration;
use rand::Rng;
use rand_distr::Distribution;

pub fn do_demo_params() {
    demo_params(Params {
        time_limit: Duration::from_secs(60),
        budget: 200000 * SOMPI_PER_KAS,
        ops_per_minute: 90,
    });
}

fn demo_params(params: Params) {
    let mut r = rand::rng();
    let mut elapsed = 0u128;
    let mut total_spend = 0;
    let mut total_ops = 0;
    while elapsed < params.time_limit.as_millis() {
        let value = params.distr_value().sample(&mut r) as u64;
        let time = params.distr_time().sample(&mut r) as u64;
        elapsed += time as u128;
        total_spend += value;
        total_ops += 1;
        println!(
            "elaspsed {}, time {}, value {}",
            elapsed,
            time,
            as_kas(value)
        );
    }
    println!(
        "total_spend: {}, total_ops: {}",
        as_kas(total_spend),
        total_ops
    );
}
