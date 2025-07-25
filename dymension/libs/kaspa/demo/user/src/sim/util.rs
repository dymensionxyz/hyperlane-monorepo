pub const SOMPI_PER_KAS: u64 = 100_000_000;
pub fn som_to_kas(sompi: u64) -> String {
    format!("{} KAS", sompi as f64 / SOMPI_PER_KAS as f64)
}

fn kaspa_addr_to_hl_hex_recipient(addr: Address) -> String {
    let addr = Address::from_str(addr).unwrap();
    let addr = addr.to_string();
    let addr = addr.to_uppercase();
    let addr = addr.replace("K", "0");
    let addr = addr.replace("A", "1");
    let addr = addr.replace("B", "2");
    let addr = addr.replace("C", "3");
    addr
}