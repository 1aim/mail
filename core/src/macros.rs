#[cfg(test)]
macro_rules! assert_not {
    //direct forward + `!`
    ($($t:tt)*) => (assert!(! $($t)*));
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
    ($name:ident, $code:block) => {
        #[test]
        fn $name() {
            let catch_block = || -> Result<(), ::error::MailError> {
                $code;
                Ok(())
            };
            (catch_block)().unwrap();
        }
    };
}
