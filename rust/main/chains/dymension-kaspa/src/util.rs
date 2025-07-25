use hyperlane_core::{
    HyperlaneDomain, HyperlaneDomainProtocol,
};

use dym_kas_core::wallet::Network;
use dym_kas_hardcode::hl::{
    HL_DOMAIN_DYM_LOCAL, HL_DOMAIN_DYM_MAINNET, HL_DOMAIN_DYM_PLAYGROUND_202507,
    HL_DOMAIN_DYM_TESTNET_BLUMBUS, HL_DOMAIN_KASPA_MAINNET, HL_DOMAIN_KASPA_TEST10,
    HL_DOMAIN_KASPA_TEST10_LEGACY,
};

/// is it a kaspa domain?
pub fn is_kas(d: &HyperlaneDomain) -> bool {
    match d {
        HyperlaneDomain::Unknown {
            domain_protocol: HyperlaneDomainProtocol::Kaspa,
            ..
        } => true,
        _ => false,
    }
}

pub fn is_dym(d: &HyperlaneDomain) -> bool {
    hub_domains().contains(&d.id())
}

pub fn domain_to_kas_network(d: &HyperlaneDomain) -> Network {
    match d {
        HyperlaneDomain::Unknown {
            domain_protocol: HyperlaneDomainProtocol::Kaspa,
            domain_id: HL_DOMAIN_KASPA_TEST10,
            ..
        } => Network::KaspaTest10,
        HyperlaneDomain::Unknown {
            domain_protocol: HyperlaneDomainProtocol::Kaspa,
            domain_id: HL_DOMAIN_KASPA_TEST10_LEGACY,
            ..
        } => Network::KaspaTest10,
        HyperlaneDomain::Unknown {
            domain_protocol: HyperlaneDomainProtocol::Kaspa,
            domain_id: HL_DOMAIN_KASPA_MAINNET,
            ..
        } => Network::KaspaMainnet,

        _ => todo!("only kaspa supported"),
    }
}

pub fn kas_domains() -> Vec<u32> {
    vec![
        HL_DOMAIN_KASPA_MAINNET,
        HL_DOMAIN_KASPA_TEST10,
        HL_DOMAIN_KASPA_TEST10_LEGACY, // TODO: remove
    ]
}

pub fn hub_domains() -> Vec<u32> {
    vec![
        HL_DOMAIN_DYM_LOCAL,
        HL_DOMAIN_DYM_MAINNET,
        HL_DOMAIN_DYM_TESTNET_BLUMBUS,
        HL_DOMAIN_DYM_PLAYGROUND_202507,
    ]
}
