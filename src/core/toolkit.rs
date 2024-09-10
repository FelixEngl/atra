pub mod utf8;
pub mod read_counter;

pub fn comp_opt<T, F: FnOnce(T, T) -> bool>(a: Option<T>, b: Option<T>, f: F) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => f(a, b),
        (None, None) => true,
        _ => false
    }
}