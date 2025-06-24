use core::{deposit::DepositFXG, confirmation::ConfirmationFXG, withdraw::WithdrawFXG};

pub fn validate_deposits(fxg: &DepositFXG) -> bool {
    true
}

pub fn validate_confirmed_withdrawals(fxg: &ConfirmationFXG) -> bool {
    true
}

pub fn validate_withdrawals(fxg: &WithdrawFXG) -> bool {
    true
}
