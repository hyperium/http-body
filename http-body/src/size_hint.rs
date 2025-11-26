/// A `Body` size hint
///
/// The default implementation returns:
///
/// * 0 for `lower`
/// * `None` for `upper`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SizeHint {
    lower: u64,
    upper: Option<u64>,
}

impl SizeHint {
    /// Returns a new `SizeHint` with default values
    #[inline]
    #[must_use]
    pub const fn new() -> SizeHint {
        Self {
            lower: 0,
            upper: None,
        }
    }

    /// Returns a new `SizeHint` with both upper and lower bounds set to the
    /// given value.
    #[inline]
    #[must_use]
    pub const fn with_exact(value: u64) -> SizeHint {
        SizeHint {
            lower: value,
            upper: Some(value),
        }
    }

    /// Returns the lower bound of data that the `Body` will yield before
    /// completing.
    #[inline]
    #[must_use]
    pub const fn lower(&self) -> u64 {
        self.lower
    }

    /// Set the value of the `lower` hint.
    ///
    /// # Panics
    ///
    /// The function panics if `value` is greater than `upper`.
    #[inline]
    pub const fn set_lower(&mut self, value: u64) {
        assert!(match self.upper {
            Some(upper) => value <= upper,
            None => true,
        });
        self.lower = value;
    }

    /// Returns the upper bound of data the `Body` will yield before
    /// completing, or `None` if the value is unknown.
    #[inline]
    #[must_use]
    pub const fn upper(&self) -> Option<u64> {
        self.upper
    }

    /// Set the value of the `upper` hint value.
    ///
    /// # Panics
    ///
    /// This function panics if `value` is less than `lower`.
    #[inline]
    pub const fn set_upper(&mut self, value: u64) {
        assert!(value >= self.lower, "`value` is less than than `lower`");

        self.upper = Some(value);
    }

    /// Returns the exact size of data that will be yielded **if** the
    /// `lower` and `upper` bounds are equal.
    #[inline]
    #[must_use]
    pub const fn exact(&self) -> Option<u64> {
        match self.upper {
            Some(upper) if self.lower == upper => Some(upper),
            Some(_) | None => None,
        }
    }

    /// Set the value of the `lower` and `upper` bounds to exactly the same.
    #[inline]
    #[must_use]
    pub const fn set_exact(&mut self, value: u64) {
        self.lower = value;
        self.upper = Some(value);
    }
}

/// Perfectly adds two `SizeHint'`s
impl core::ops::Add for SizeHint {
    type Output = SizeHint;

    fn add(self, rhs: Self) -> Self::Output {
        SizeHint {
            lower: self.lower() + rhs.lower(),
            upper: self
                .upper()
                .and_then(|this| rhs.upper().map(|rhs| this + rhs)),
        }
    }
}

#[test]
fn size_init_new_default() {
    let new = SizeHint::new();
    assert_eq!(new.lower(), 0);
    assert_eq!(new.upper(), None);
    assert_eq!(new.exact(), None);

    let def = SizeHint::default();
    assert_eq!(def.lower(), 0);
    assert_eq!(def.upper(), None);
    assert_eq!(def.exact(), None);
}

/// Asserts that SizeHint addition is perfect with a basic proof
#[test]
fn size_hint_addition_proof() {
    /// Converts a SizeHint to a tuple for equality checks and matching
    fn to_parts(s: SizeHint) -> (u64, Option<u64>) {
        (s.lower(), s.upper())
    }

    // assuming addition itself is perfect, there are 3 distinct states:
    // (_, Some(_)) + (_, Some(_)) => (_ + _, Some(_ + _))
    // (_, Some(_)) + (_, None) => (_ + _, None)
    // (_, None) + (_, None) => (_ + _, None)
    //
    // we can assert this in the typesystem! (and name them for our tests)
    match (to_parts(SizeHint::new()), to_parts(SizeHint::new())) {
        ((_, Some(_)), (_, Some(_))) => {} // 1
        ((_, None), (_, None)) => {}       // 2

        // note that these cases are identical if we can prove lhs + rhs is equivalent to rhs + lhs
        // see below, we do prove that!
        ((_, Some(_)), (_, None)) => {} // 3
        ((_, None), (_, Some(_))) => {}
    }
    //
    // Additionally, we assert a with_exact remains intact if we add two with_exact's together
    //
    // Additionally, we assert that all operations are equivalent if we do a + b vs b + a

    // asserts a + b == b + a == eq
    macro_rules! reciprocal_add_eq {
        ($a:expr, $b:expr, $eq:expr) => {
            assert_eq!(to_parts(($a.clone() + $b.clone())), $eq);
            assert_eq!(to_parts(($b.clone() + $a.clone())), $eq);
        };
    }

    // note that we use increasing powers of two every time we fetch a number, this ensures all
    // numbers will add uniquely

    let exact_1 = SizeHint::with_exact(1);
    let exact_2 = SizeHint::with_exact(2);

    // with_exact
    reciprocal_add_eq!(exact_1, exact_2, to_parts(SizeHint::with_exact(1 + 2)));

    let some_lhs = SizeHint {
        lower: 4,
        upper: Some(8),
    };

    let some_rhs = SizeHint {
        lower: 16,
        upper: Some(32),
    };

    // case 1
    reciprocal_add_eq!(some_lhs, some_rhs, (4 + 16, Some(8 + 32)));

    let none_lhs = SizeHint {
        lower: 64,
        upper: None,
    };

    let none_rhs = SizeHint {
        lower: 128,
        upper: None,
    };

    // case 2
    reciprocal_add_eq!(none_lhs, none_rhs, (64 + 128, None));

    // case 3
    reciprocal_add_eq!(some_lhs, none_rhs, (4 + 128, None));
}

/// Asserts that some "real data" gets passed through without issue
#[test]
fn size_hint_addition_basic() {
    let exact_l = SizeHint::with_exact(20);
    let exact_r = SizeHint::with_exact(5);

    assert_eq!(Some(25), (exact_l + exact_r).exact());

    let inexact_l = SizeHint {
        lower: 25,
        upper: None,
    };
    let inexact_r = SizeHint {
        lower: 10,
        upper: Some(50),
    };

    let inexact = inexact_l + inexact_r;

    assert_eq!(inexact.lower(), 35);
    assert_eq!(inexact.upper(), None);

    let exact_inexact = exact_l + inexact_r;

    assert_eq!(exact_inexact.lower(), 30);
    assert_eq!(exact_inexact.upper(), Some(70));

    // same as previous but reversed operation order
    let inexact_exact = inexact_r + exact_l;

    assert_eq!(inexact_exact.lower(), 30);
    assert_eq!(inexact_exact.upper(), Some(70));
}
