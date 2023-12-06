use rearch::{Capsule, CapsuleHandle, CapsuleKey, Container};

struct FibonacciCapsule(u8);
impl Capsule for FibonacciCapsule {
    type Data = u128;

    fn build(&self, CapsuleHandle { mut get, .. }: CapsuleHandle) -> Self::Data {
        let Self(n) = self;
        match n {
            0 => 0,
            1 => 1,
            n => get.get(Self(n - 1)) + get.get(Self(n - 2)),
        }
    }

    fn eq(old: &Self::Data, new: &Self::Data) -> bool {
        old == new
    }

    fn key(&self) -> CapsuleKey {
        let Self(id) = self;
        id.to_le_bytes().as_ref().to_owned().into()
    }
}

fn main() {
    let container = Container::new();
    println!(
        "The 100th fibonacci number is {}",
        container.read(FibonacciCapsule(100)),
    );
}

#[test]
fn fib_number_is_correct() {
    let container = Container::new();
    assert_eq!(
        container.read(FibonacciCapsule(100)),
        354_224_848_179_261_915_075
    );
}
