use std::thread;

use crossbeam_channel::{bounded, Sender, Receiver, TrySendError};

use crate::types::{Meta, Screenshot};
use crate::parser::parse;

const INCOMING_QUEUE_SIZE: usize = 3;
const OUTGOING_QUEUE_SIZE: usize = 3;

pub fn new_processor_pipeline() -> (Sender<Screenshot>, Receiver<Screenshot>) {
    let (input_tx, input_rx) = bounded::<Screenshot>(INCOMING_QUEUE_SIZE);
    let (output_tx, output_rx) = bounded::<Screenshot>(OUTGOING_QUEUE_SIZE);
    thread::spawn(move || {
        worker(input_rx, output_tx);
    });
    (input_tx, output_rx)
}

fn worker(rx: Receiver<Screenshot>, tx: Sender<Screenshot>) {
    let mut prev = Meta::empty();
    for mut ss in rx {
        ss.set_received();
        ss.meta = parse(&ss.image);
        ss.set_parsed();
        if !prev.same(&ss.meta) {
            println!("It seems something happened: {:?}", &ss.meta);
            prev = ss.meta.clone();
            match tx.try_send(ss) {
                Ok(()) => {},
                Err(TrySendError::Full(_)) => {eprintln!("processed screenshot has been dropped");},
                Err(TrySendError::Disconnected(_)) => {
                    eprintln!("worker disconnected");
                    break;
                },
            }
        } else {
            prev = ss.meta.clone();
            drop(ss);
        }
    }
}

