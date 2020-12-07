macro_rules! sep_for {
    ($var:ident in $iter:expr; sep $sep:block; $($rem:tt)* ) => {{
        let mut first = true;
        for $var in $iter {
            if first { first = false; }
            else {
                $sep
            }
            $( $rem )*
        }
    }}
}

macro_rules! deref0 {
    (+mut $name:ident => $tp:ty) => {
        deref0! {-mut $name => $tp }
        impl ::std::ops::DerefMut for $name {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }
    };
    (-mut $name:ident => $tp:ty) => {
        impl ::std::ops::Deref for $name {
            type Target = $tp;
            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }
    };
}

#[cfg(test)]
macro_rules! assert_ok {
    ($val:expr) => {{
        match $val {
            Ok(res) => res,
            Err(err) => panic!("expected Ok(..) got Err({:?})", err),
        }
    }};
    ($val:expr, $ctx:expr) => {{
        match $val {
            Ok(res) => res,
            Err(err) => panic!("expected Ok(..) got Err({:?}) [ctx: {:?}]", err, $ctx),
        }
    }};
}

#[cfg(test)]
macro_rules! assert_err {
    ($val:expr) => {{
        match $val {
            Ok(val) => panic!("expected Err(..) got Ok({:?})", val),
            Err(err) => err,
        }
    }};
    ($val:expr, $ctx:expr) => {{
        match $val {
            Ok(val) => panic!("expected Err(..) got Ok({:?}) [ctx: {:?}]", val, $ctx),
            Err(err) => err,
        }
    }};
}

#[cfg(test)]
macro_rules! test {
    ($name:ident $code:block) => {
        #[test]
        fn $name() {
            fn inner() -> Result<(), ::failure::Error> {
                $code;
                #[allow(unreachable_code)]
                Ok(())
            }
            inner().unwrap();
        }
    };
}
