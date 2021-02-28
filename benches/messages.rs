use bit_vec::BitVec;
use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion};
use thiserror::Error;

use appliance::{Appliance, ApplianceHandle, Handler, DEFAULT_EXECUTOR};

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    ApplianceError(#[from] appliance::Error),
    #[error(transparent)]
    IndexError(#[from] messages::InvalidIndex),
}

mod messages {
    use {appliance::Message, thiserror::Error};

    #[derive(Debug, Clone, Error)]
    #[error("current bitvec index is different from the one in the message")]
    pub struct InvalidIndex;

    pub struct SetValue(pub usize);

    impl Message for SetValue {
        type Result = Result<(), InvalidIndex>;
    }

    pub struct Validate;

    impl Message for Validate {
        type Result = bool;
    }

    pub struct Reset;

    impl Message for Reset {
        type Result = ();
    }
}

type BenchAppliance = Appliance<'static, State>;

impl Handler<messages::SetValue> for BenchAppliance {
    fn handle(
        &mut self,
        messages::SetValue(i): messages::SetValue,
    ) -> Result<(), messages::InvalidIndex> {
        if self.state().index == i {
            self.state().bit_lane.set(i, true);
            self.state().index += 1;
            Ok(())
        } else {
            Err(messages::InvalidIndex)
        }
    }
}

impl Handler<messages::Validate> for BenchAppliance {
    fn handle(&mut self, _m: messages::Validate) -> bool {
        self.state().bit_lane.all()
    }
}

impl Handler<messages::Reset> for BenchAppliance {
    fn handle(&mut self, _m: messages::Reset) {
        self.state().bit_lane.clear();
    }
}

#[derive(Debug, Clone)]
struct State {
    index: usize,
    bit_lane: BitVec,
}

impl State {
    pub fn new(size: usize) -> State {
        State {
            index: 0,
            bit_lane: BitVec::from_elem(size, false),
        }
    }
}

pub fn set_bitvec(c: &mut Criterion) {
    let msg_count: usize = black_box(1_000_000);
    let benchmark = |handle: ApplianceHandle<BenchAppliance>| {
        for i in 0..msg_count {
            handle.send(messages::SetValue(i))?;
        }
        let success = handle.send_and_wait_sync(messages::Validate, None)?;
        if !success {
            panic!("Validation has failed. Not all bits are set.")
        }
        Ok::<(), Error>(())
    };
    c.bench_function("set_bitvec", |b| {
        b.iter_batched(
            || Appliance::new_unbounded(&DEFAULT_EXECUTOR, State::new(msg_count)),
            benchmark,
            BatchSize::SmallInput,
        )
    });
}

criterion_group!(
    name = benches;
    config = Criterion::default().confidence_level(0.99).significance_level(0.01);
    targets = set_bitvec
);
criterion_main!(benches);
