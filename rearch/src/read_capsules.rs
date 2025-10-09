use crate::{ArcContainerStore, Capsule, Container, CreateCapsuleId};

/// A list of capsules with cloneable data.
/// This is either a singular capsule, like `foo_capsule`,
/// or a tuple, like `(foo_capsule, bar_capsule)`.
pub trait CapsulesWithCloneRead {
    type Data;
    fn read(self, container: &Container) -> Self::Data;
}
macro_rules! generate_capsule_list_impl {
    ($($C:ident),+) => {
        paste::paste! {
            #[allow(non_snake_case, unused_parens)]
            impl<$($C: Capsule),*> CapsulesWithCloneRead for ($($C),*) where $($C::Data: Clone),* {
                type Data = ($($C::Data),*);
                fn read(self, container: &Container) -> Self::Data {
                    let ($([<i $C>]),*) = self;
                    let attempted_read_capsules = {
                        let txn = container.0.read_txn();
                        ($(txn.try_read(&[<i $C>])),*)
                    };
                    if let ($(Some([<i $C>])),*) = attempted_read_capsules {
                        ($([<i $C>]),*)
                    } else {
                        let mut txn = container.0.write_txn();
                        ($(txn.read_or_init([<i $C>])),*)
                    }
                }
            }
        }
    };
}
generate_capsule_list_impl!(A);
generate_capsule_list_impl!(A, B);
generate_capsule_list_impl!(A, B, C);
generate_capsule_list_impl!(A, B, C, D);
generate_capsule_list_impl!(A, B, C, D, E);
generate_capsule_list_impl!(A, B, C, D, E, F);
generate_capsule_list_impl!(A, B, C, D, E, F, G);
generate_capsule_list_impl!(A, B, C, D, E, F, G, H);

/// A list of capsules that can be read via a ref.
/// This is either a singular capsule, like `foo_capsule`,
/// or a tuple, like `(foo_capsule, bar_capsule)`.
pub trait CapsulesWithRefRead {
    type Data<'a>;
    fn read<Callback, CallbackReturn>(
        self,
        container: &Container,
        callback: Callback,
    ) -> CallbackReturn
    where
        Callback: FnOnce(Self::Data<'_>) -> CallbackReturn;
}
macro_rules! generate_capsule_list_impl {
    ($($C:ident),+) => {
        paste::paste! {
            #[allow(non_snake_case, unused_parens, clippy::double_parens)]
            impl<$($C: Capsule),*> CapsulesWithRefRead for ($($C),*) {
                type Data<'a> = ($(&'a $C::Data),*);
                fn read<Callback, CallbackReturn>(
                    self,
                    container: &Container,
                    callback: Callback,
                ) -> CallbackReturn
                where
                    Callback: FnOnce(Self::Data<'_>) -> CallbackReturn,
                {
                    let ($([<capsule $C>]),*) = self;
                    let ($([<id $C>]),*) = ($([<capsule $C>].id()),*);
                    let read_guard = Some(container.0.read_txn())
                        $(  .filter(|txn| txn.try_read_ref(&[<capsule $C>]).is_some())  )*
                        .unwrap_or_else(|| {
                            let mut txn = container.0.write_txn();
                            $(  txn.ensure_initialized([<capsule $C>]);  )*
                            txn.downgrade()
                        })
                        .data;
                        callback((
                            $(
                                read_guard
                                    .get(&[<id $C>])
                                    .map(crate::downcast_capsule_data::<$C>)
                                    .expect("Ensured initialization above")
                            ),*
                        ))
                }
            }
        }
    };
}
generate_capsule_list_impl!(A);
generate_capsule_list_impl!(A, B);
generate_capsule_list_impl!(A, B, C);
generate_capsule_list_impl!(A, B, C, D);
generate_capsule_list_impl!(A, B, C, D, E);
generate_capsule_list_impl!(A, B, C, D, E, F);
generate_capsule_list_impl!(A, B, C, D, E, F, G);
generate_capsule_list_impl!(A, B, C, D, E, F, G, H);

#[cfg(test)]
mod tests {
    use crate::{CapsuleHandle, Container};

    fn my_capsule(_: CapsuleHandle) -> u8 {
        123
    }

    #[test]
    fn container_ref_read() {
        let mut callback_called = false;
        Container::new().read_ref(my_capsule, |data| {
            callback_called = true;
            assert_eq!(data, &123);
        });
        assert!(callback_called);
    }

    #[test]
    fn container_ref_read_multi() {
        let mut callback_called = false;
        Container::new().read_ref((my_capsule, my_capsule), |(data1, data2)| {
            callback_called = true;
            assert_eq!(data1, &123);
            assert_eq!(data2, &123);
        });
        assert!(callback_called);
    }

    #[test]
    fn container_clone_read() {
        assert_eq!(Container::new().read(my_capsule), 123);
    }

    #[test]
    fn container_clone_read_multi() {
        assert_eq!(Container::new().read((my_capsule, my_capsule)), (123, 123));
    }
}
