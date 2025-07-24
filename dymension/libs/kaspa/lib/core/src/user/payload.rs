use hyperlane_core::{HyperlaneMessage, TokenMessage, H256, U256};

pub fn make_deposit_payload(
    domain_kas: u32,
    token_kas_placeholder: H256,
    domain_hub: u32,
    token_hub: H256,
    hub_user_addr_hub: H256,
    amt: u64,
) -> Vec<u8> {
    let mut m = HyperlaneMessage::default();
    m.origin = domain_kas;
    m.sender = token_kas_placeholder;
    m.destination = domain_hub;
    m.recipient = token_hub;
    m.body = vec![];
    let meta = make_deposit_payload_meta();
    let token_message = TokenMessage::new(hub_user_addr_hub, U256::from(amt), meta);
    let mut buf = vec![];
    token_message.write_to(&mut buf).unwrap();
    m.body = buf;
    m.write_to(&mut buf).unwrap();
    buf
}

fn make_deposit_payload_meta() -> Vec<u8> {}
