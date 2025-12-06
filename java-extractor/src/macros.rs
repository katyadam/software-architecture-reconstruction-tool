#[macro_export]
macro_rules! s {
    ($x:expr) => {
        $x.to_string()
    };
}

#[macro_export]
macro_rules! strs {
    ( $( $x:expr ),* ) => {
        vec![ $( $x.to_string() ),* ]
    };
}
