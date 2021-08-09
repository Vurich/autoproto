#![feature(generic_associated_types, trivial_bounds)]
#![allow(dead_code)]

#[derive(Copy, Clone, PartialEq, autoproto::IsDefault, Default, Debug, autoproto::Message)]
struct Unit;

#[derive(Copy, Clone, PartialEq, autoproto::IsDefault, Default, Debug, autoproto::Message)]
struct Foo<A, B>(#[autoproto(tag = 4)] A, #[autoproto(tag = 5)] B);

#[derive(Copy, Clone, PartialEq, autoproto::IsDefault, Default, Debug, autoproto::Message)]
struct SomeStruct<A, B> {
    a: A,
    b: B,
}

#[derive(Copy, Clone, PartialEq, autoproto::IsDefault, Default, Debug, autoproto::Message)]
#[autoproto(transparent)]
struct Wrapper(SomeStruct<Foo<u32, u64>, SomeStruct<f32, Unit>>);

trait DummyOne {}
trait DummyTwo {}
trait DummyThree {}

#[derive(Copy, Clone, PartialEq, Debug, autoproto::Message)]
enum Oneof<A: DummyOne, B: DummyTwo, C: DummyThree> {
    Nothing,
    NothingWithBrackets(),
    NothingWithBraces {},
    One(A),
    OneWithBraces { a: A },
    Two(A, B),
    TwoWithBraces { a: A, b: B },
    Three(A, B, C),
    ThreeWithBraces { a: A, b: B, c: C },
}

impl<A: DummyOne, B: DummyTwo, C: DummyThree> Default for Oneof<A, B, C> {
    fn default() -> Self {
        Self::Nothing
    }
}

impl<A: DummyOne, B: DummyTwo, C: DummyThree> autoproto::IsDefault for Oneof<A, B, C> {
    fn is_default(&self) -> bool {
        match self {
            Self::Nothing => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DummyOne, DummyThree, DummyTwo, Foo, SomeStruct, Unit, Wrapper};
    use autoproto::prost::Message;

    use quickcheck::TestResult;
    use quickcheck_macros::quickcheck;

    type WithOptionals = SomeStruct<Option<Foo<u32, u64>>, Option<SomeStruct<f32, Option<Unit>>>>;

    impl From<Wrapper> for WithOptionals {
        fn from(other: Wrapper) -> Self {
            SomeStruct {
                a: Some(other.0.a),
                b: Some(SomeStruct {
                    a: other.0.b.a,
                    b: Some(other.0.b.b),
                }),
            }
        }
    }

    static_assertions::assert_impl_all!(Wrapper: Message, autoproto::Proto);

    const _: fn() = || {
        fn assert_impl<T: Message + autoproto::Proto>() {}

        fn assert_foo_and_somestruct_impl<
            A: PartialEq + Default + std::fmt::Debug + Send + Sync + autoproto::Proto,
            B: PartialEq + Default + std::fmt::Debug + Send + Sync + autoproto::Proto,
        >() {
            assert_impl::<Foo<A, B>>();
            assert_impl::<SomeStruct<A, B>>();
        }

        fn assert_oneof_impl<
            A: DummyOne + Default + std::fmt::Debug + Send + Sync + autoproto::Proto,
            B: DummyTwo + Default + std::fmt::Debug + Send + Sync + autoproto::Proto,
            C: DummyThree + Default + std::fmt::Debug + Send + Sync + autoproto::Proto,
        >() {
            assert_impl::<super::Oneof<A, B, C>>();
        }
    };

    mod unrolled_wrapper {
        use super::Message;

        #[derive(Copy, Clone, Message)]
        pub struct Inner1 {
            #[prost(uint32, tag = "4")]
            pub a: u32,
            #[prost(uint64, tag = "5")]
            pub b: u64,
        }

        #[derive(Copy, Clone, Message)]
        pub struct Inner2 {
            #[prost(float, tag = "1")]
            pub a: f32,
            #[prost(message, tag = "2")]
            pub b: Option<()>,
        }

        #[derive(Copy, Clone, Message)]
        pub struct Outer {
            #[prost(message, tag = "1")]
            pub a: Option<Inner1>,
            #[prost(message, tag = "2")]
            pub b: Option<Inner2>,
        }
    }

    fn round_trip<T: Message + Default + PartialEq>(proto: &T) -> Vec<u8> {
        let mut out = T::default();
        let encoded = proto.encode_to_vec();

        out.merge(&encoded[..]).unwrap();

        assert_eq!(*proto, out);

        encoded
    }

    fn make_wrapper(a: u32, b: u64, c: f32) -> Wrapper {
        Wrapper(SomeStruct {
            a: Foo(a, b),
            b: SomeStruct { a: c, b: Unit },
        })
    }

    fn make_with_optionals(a: Option<(u32, u64)>, b: Option<(f32, Option<()>)>) -> WithOptionals {
        SomeStruct {
            a: a.map(|(a, b)| Foo(a, b)),
            b: b.map(|(a, b)| SomeStruct {
                a,
                b: b.map(|()| Unit),
            }),
        }
    }

    #[quickcheck]
    fn round_trip_test((a, b, c): (u32, u64, f32)) -> TestResult {
        if c.is_nan() {
            TestResult::discard()
        } else {
            round_trip(&make_wrapper(a, b, c));
            TestResult::passed()
        }
    }

    #[quickcheck]
    fn round_trip_optional((a, b): (Option<(u32, u64)>, Option<(f32, Option<()>)>)) -> TestResult {
        if let Some((f, _)) = b {
            if f.is_nan() {
                return TestResult::discard();
            }
        }

        round_trip(&make_with_optionals(a, b));

        TestResult::passed()
    }

    fn test_same_as_prost(proto: WithOptionals) {
        let prost_inner_1 = proto.a.map(|Foo(a, b)| unrolled_wrapper::Inner1 { a, b });
        let prost_inner_2 = proto.b.map(|SomeStruct { a, b }| unrolled_wrapper::Inner2 {
            a,
            b: b.map(|Unit| ()),
        });
        let prost_outer = unrolled_wrapper::Outer {
            a: prost_inner_1,
            b: prost_inner_2,
        };

        fn encode_or_empty<T: Message>(val: Option<T>) -> Vec<u8> {
            val.map(|val| val.encode_to_vec()).unwrap_or_default()
        }

        assert_eq!(encode_or_empty(proto.a), encode_or_empty(prost_inner_1));
        assert_eq!(encode_or_empty(proto.b), encode_or_empty(prost_inner_2));
        assert_eq!(proto.encode_to_vec(), prost_outer.encode_to_vec());
    }

    #[quickcheck]
    fn same_as_prost((a, b, c): (u32, u64, f32)) {
        test_same_as_prost(make_wrapper(a, b, c).into());
    }

    #[quickcheck]
    fn same_as_prost_with_optionals((a, b): (Option<(u32, u64)>, Option<(f32, Option<()>)>)) {
        test_same_as_prost(make_with_optionals(a, b));
    }

    #[quickcheck]
    fn with_optionals_same_as_without((a, b, c): (u32, u64, f32)) {
        let wrapper = make_wrapper(a, b, c);

        assert_eq!(
            wrapper.encode_to_vec(),
            WithOptionals::from(wrapper).encode_to_vec()
        );
    }

    #[test]
    fn zero_same_as_prost() {
        test_same_as_prost(
            Wrapper(SomeStruct {
                a: Foo(0, 0),
                b: SomeStruct { a: 0., b: Unit },
            })
            .into(),
        );
    }

    type FirstA = Foo<u64, u32>;
    type FirstB = Foo<u32, f32>;
    type SecondA = SomeStruct<Foo<f32, u32>, Foo<u32, u64>>;
    type SecondB = SomeStruct<Foo<u32, u64>, Foo<f32, u64>>;

    #[derive(Copy, Clone, PartialEq, Debug, autoproto::Message)]
    enum Oneof {
        Nothing,
        First { a: FirstA, b: FirstB },
        Second { a: SecondA, b: SecondB },
    }

    impl Default for Oneof {
        fn default() -> Self {
            Self::Nothing
        }
    }

    impl autoproto::IsDefault for Oneof {
        fn is_default(&self) -> bool {
            if let Self::Nothing = self {
                true
            } else {
                false
            }
        }
    }

    #[quickcheck]
    fn oneof_same_as_with_optional_fields(args: Option<(bool, u32, u64, f32)>) {
        #[derive(Copy, Clone, PartialEq, Debug, autoproto::Message)]
        struct OptionalFields {
            nothing: Option<()>,
            a: Option<SomeStruct<FirstA, FirstB>>,
            b: Option<SomeStruct<SecondA, SecondB>>,
        }

        impl Default for OptionalFields {
            fn default() -> Self {
                Self {
                    nothing: Some(()),
                    a: None,
                    b: None,
                }
            }
        }

        impl autoproto::IsDefault for OptionalFields {
            fn is_default(&self) -> bool {
                self.nothing.is_some() && self.a.is_none() && self.b.is_none()
            }
        }

        let (oneof, optional) = match args {
            None => (Oneof::Nothing, OptionalFields::default()),
            Some((true, uint32, uint64, float)) => (
                Oneof::First {
                    a: Foo(uint64, uint32),
                    b: Foo(uint32, float),
                },
                OptionalFields {
                    a: Some(SomeStruct {
                        a: Foo(uint64, uint32),
                        b: Foo(uint32, float),
                    }),
                    b: None,
                    nothing: None,
                },
            ),
            Some((false, uint32, uint64, float)) => (
                Oneof::Second {
                    a: SomeStruct {
                        a: Foo(float, uint32),
                        b: Foo(uint32, uint64),
                    },
                    b: SomeStruct {
                        a: Foo(uint32, uint64),
                        b: Foo(float, uint64),
                    },
                },
                OptionalFields {
                    a: None,
                    b: Some(SomeStruct {
                        a: SomeStruct {
                            a: Foo(float, uint32),
                            b: Foo(uint32, uint64),
                        },
                        b: SomeStruct {
                            a: Foo(uint32, uint64),
                            b: Foo(float, uint64),
                        },
                    }),
                    nothing: None,
                },
            ),
        };

        assert_eq!(oneof.encode_to_vec(), optional.encode_to_vec());
    }

    #[quickcheck]
    fn oneof_same_as_prost(args: Option<(bool, u32, u64, f32)>) {
        #[derive(::prost::Message)]
        struct Outer {
            #[prost(oneof = "Inner", tags = "1, 2, 3")]
            inner: Option<Inner>,
        }

        #[derive(::prost::Oneof)]
        enum Inner {
            #[prost(message, tag = 1)]
            Nothing(()),
            #[prost(message, tag = 2)]
            First(SomeStruct<FirstA, FirstB>),
            #[prost(message, tag = 3)]
            Second(SomeStruct<SecondA, SecondB>),
        }

        fn to_prost(val: &Oneof) -> Outer {
            let inner = match *val {
                Oneof::Nothing => Inner::Nothing(()),
                Oneof::First { a, b } => Inner::First(SomeStruct { a, b }),
                Oneof::Second { a, b } => Inner::Second(SomeStruct { a, b }),
            };

            Outer { inner: Some(inner) }
        }

        let oneof = match args {
            None => Oneof::Nothing,
            Some((true, uint32, uint64, float)) => Oneof::First {
                a: Foo(uint64, uint32),
                b: Foo(uint32, float),
            },
            Some((false, uint32, uint64, float)) => Oneof::Second {
                a: SomeStruct {
                    a: Foo(float, uint32),
                    b: Foo(uint32, uint64),
                },
                b: SomeStruct {
                    a: Foo(uint32, uint64),
                    b: Foo(float, uint64),
                },
            },
        };

        let prost = to_prost(&oneof);

        assert_eq!(oneof.encode_to_vec(), prost.encode_to_vec());
    }

    #[quickcheck]
    fn repeated_ints_same_as_prost(u32s: Vec<u32>, u64s: Vec<u64>) {
        #[derive(::prost::Message)]
        struct ProstMsg {
            #[prost(repeated, uint32, tag = 1)]
            u32s: Vec<u32>,
            #[prost(repeated, uint64, tag = 2)]
            u64s: Vec<u64>,
        }

        #[derive(autoproto::IsDefault, PartialEq, Default, Debug, autoproto::Message)]
        struct AutoprotoMsg {
            #[autoproto(tag = 1)]
            u32s: Vec<u32>,
            #[autoproto(tag = 2)]
            u64s: Vec<u64>,
        }

        let prost_msg = ProstMsg {
            u32s: u32s.clone(),
            u64s: u64s.clone(),
        };
        let autoproto_msg = AutoprotoMsg { u32s, u64s };

        assert_eq!(prost_msg.encode_to_vec(), autoproto_msg.encode_to_vec());
    }

    #[quickcheck]
    fn repeated_messages_same_as_prost(a: Vec<(u32, u64)>, b: Vec<(f32, f64)>) {
        #[derive(::prost::Message)]
        struct ProstMsg {
            #[prost(repeated, message, tag = 1)]
            a: Vec<SomeStruct<u32, u64>>,
            #[prost(repeated, message, tag = 2)]
            b: Vec<Foo<f32, f64>>,
        }

        #[derive(autoproto::IsDefault, PartialEq, Default, Debug, autoproto::Message)]
        struct AutoprotoMsg {
            #[autoproto(tag = 1)]
            a: Vec<SomeStruct<u32, u64>>,
            #[autoproto(tag = 2)]
            b: Vec<Foo<f32, f64>>,
        }

        let (a, b): (Vec<_>, Vec<_>) = (
            a.into_iter().map(|(a, b)| SomeStruct { a, b }).collect(),
            b.into_iter().map(|(a, b)| Foo(a, b)).collect(),
        );

        let prost_msg = ProstMsg {
            a: a.clone(),
            b: b.clone(),
        };
        let autoproto_msg = AutoprotoMsg { a, b };

        assert_eq!(prost_msg.encode_to_vec(), autoproto_msg.encode_to_vec());
    }
}
