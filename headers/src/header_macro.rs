pub use soft_ascii_string::SoftAsciiStr as _SoftAsciiStr;

/// Defines a new header types with given type name, filed name and component
///
/// Note that the name is not checked/validated, it has to be ascii, a valid
/// header field name AND has to comply with the naming schema (each word
/// separated by `'-'` starts with a capital letter and no capital letter
/// follow, e.g. "Message-Id" is ok **but "Message-ID" isn't**).
///
/// This macro will create a test which will check if the used field names
/// are actually valid and appears only once (_per def_header macro call_)
/// so as long as test's are run any invalid name will be found.
///
/// Note that even if a invalid name was used and test where ignored/not run
/// this will _not_ cause an rust safety issue, but can still cause bugs under
/// some circumstances (e.g. if you have multiple differing definitions of the
/// same header with different spelling (at last one failed the test) like e.g.
/// when you override default implementations of fields).
///
/// The macros expects following items:
///
/// 1. `test_name`, which is the name the auto-generated test will have
/// 2. `scope`, the scope all components are used with, this helps with some
///    name collisions. Use `self` to use the current scope.
/// 3. a list of header definitions consisting of:
///
///    1. `<typename>` the name the type of the header will have, i.e. the name of a zero-sized
///       struct which will be generated
///    3. `unchecked` a hint to make people read the documentation and not forget the the
///       folowing data is `unchecked` / only vaidated in the auto-generated test
///    4. `"<header_name>"` the header name in a syntax using `'-'` to serperate words,
///       also each word has to start with a capital letter and be followed by lowercase
///       letters additionaly to being a valid header field name. E.g. "Message-Id" is
///       ok, but "Message-ID" is not. (Note that header field name are on itself ignore
///       case, but by enforcing a specific case in the encoder equality checks can be
///       done on byte level, which is especially usefull for e.g. placing them as keys
///       into a HashMap or for performance reasons.
///    5. `<component>` the name of the type to use ing `scope` a the component type of
///       the header. E.g. `Unstructured` for an unstructured header field (which still
///       support Utf8 through encoded words)
///    6. `None`/`maxOne`/`<name>`, None, maxOne or the name of a validator function.
///       The validator function is used to validate some contextual limitations a header field
///       might have, like that it can appear at most one time, or that if a `From` with multiple
///       mailboxes is given a `Sender` field needs to be given too.
///       If `maxOne` is used it will automatically generate a function which makes sure that
///       the header appears at most one time. This validator functions are used _after_ creating
///       a header map but before using it to encode a mail (or anywhere in between if you want to).
///       Note that validators are kept separate from the headers and might be run even if the header
///       does not appear in the header map passed to the validator, as such if a validator can not find
///       the header it should validate or if it finds it but it has an unexpected type it _must not_
///       create an error.
///
/// # Example
///
/// ```norun
/// def_headers! {
///     // the name of the auto-generated test
///     test_name: validate_header_names,
///     // the scope from which all components should be imported
///     // E.g. `DateTime` refers to `components::DateTime`.
///     scope: components,
///     // definitions of the headers or the form
///     // <type_name>, unchecked { <struct_name> }, <component>, <validator>
///     Date,     unchecked { "Date"          },  DateTime,       maxOne,
///     From,     unchecked { "From"          },  MailboxList,    validator_from,
///     Subject,  unchecked { "Subject"       },  Unstructured,   maxOne,
///     Comments, unchecked { "Comments"      },  Unstructured,   None,
/// }
/// ```
#[macro_export]
macro_rules! def_headers {
    (
        test_name: $tn:ident,
        scope: $scope:ident,
        $(
            $(#[$attr:meta])*
            $name:ident, unchecked { $hname:tt }, $component:ident,
              $maxOne:ident, $validator:ident
        ),+
    ) => (
        $(
            $(#[$attr])*
            #[derive(Default, Copy, Clone)]
            pub struct $name;

            impl $crate::HeaderKind for $name {

                type Component = $scope::$component;

                fn name() -> $crate::HeaderName {
                    let as_str: &'static str = $hname;
                    $crate::HeaderName::from_ascii_unchecked( as_str )
                }

                const MAX_ONE: bool = def_headers!{ _PRIV_mk_max_one $maxOne };
                const VALIDATOR: ::std::option::Option<$crate::map::HeaderMapValidator> =
                        def_headers!{ _PRIV_mk_validator $validator };
            }

            def_headers!{ _PRIV_mk_marker_impl $name, $maxOne }
        )+

        //TODO warn if header type name and header name diverges
        // (by stringifying the type name and then ziping the
        //  array of type names with header names removing
        //  "-" from the header names and comparing them to
        //  type names)


        #[cfg(test)]
        const HEADER_NAMES: &[ &str ] = &[ $(
            $hname
        ),+ ];

        #[test]
        fn $tn() {
            use std::collections::HashSet;
            use $crate::__internals::encoder::EncodableInHeader;

            let mut name_set = HashSet::new();
            for name in HEADER_NAMES {
                if !name_set.insert(name) {
                    panic!("name appears more than one time in same def_headers macro: {:?}", name);
                }
            }
            fn can_be_trait_object<EN: EncodableInHeader>( v: Option<&EN> ) {
                let _ = v.map( |en| en as &EncodableInHeader );
            }
            $(
                can_be_trait_object::<$scope::$component>( None );
            )+
            for name in HEADER_NAMES {
                let res = $crate::HeaderName::new(
                    $crate::soft_ascii_string::SoftAsciiStr::from_str(name).unwrap()
                );
                if res.is_err() {
                    panic!( "invalid header name: {:?} ({:?})", name, res.unwrap_err() );
                }
            }
        }
    );
    (_PRIV_mk_marker_impl $name:ident, multi) => ();
    (_PRIV_mk_marker_impl $name:ident, maxOne) => (
        impl $crate::MaxOneMarker for $name {}
    );
    (_PRIV_mk_marker_impl $name:ident, $other:ident) => (def_headers!{ _PRIV_max_one_err $other });
    (_PRIV_mk_validator None) => ({ None });
    (_PRIV_mk_validator $validator:ident) => ({ Some($validator) });
    (_PRIV_mk_max_one multi) => ({ false });
    (_PRIV_mk_max_one maxOne) => ({ true });
    (_PRIV_mk_max_one $other:ident) => (def_headers!{ _PRIV_max_one_err $other });
    (_PRIV_max_one_err $other:ident) => (
        compile_error!(concat!(
            "maxOne column can only contain `maxOne` or `multi`, got: ",
            stringify!($other)
        ));
    );
}
