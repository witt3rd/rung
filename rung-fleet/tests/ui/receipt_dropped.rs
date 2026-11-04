//! `an-unconsumed-receipt-does-not-compile`: a function that sends and drops
//! its promise or its receipt is refused.
#![deny(unused_must_use)]
use rung_fleet::delivery::{send, Delivery, Receipt, Target};

fn forgets_the_promise(target: &Target, d: &Delivery) {
    send(target, d).unwrap();
}

fn forgets_the_receipt(r: Result<Receipt, ()>) {
    if let Ok(receipt) = r {
        receipt;
    }
}

fn main() {
    let _ = (forgets_the_promise, forgets_the_receipt);
}
