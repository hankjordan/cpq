use cpq::*;

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Location {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

fn main() {
    let cpq: ConcurrentPriorityQueue<Location, i32> = ConcurrentPriorityQueue::new();

    let a = Location::new(0, 0, 0);
    let b = Location::new(0, 0, 0);
    let c = Location::new(5, 0, 0);
    let d = Location::new(0, 6, 0);
    
    cpq.push(a, 0);
    cpq.push(b, 2);
    cpq.push(c, 7);
    cpq.push(d, 2);

    assert_eq!(cpq.pop(), Some(a));
    assert_eq!(cpq.pop(), Some(b));
    assert_eq!(cpq.pop(), Some(d));
    assert_eq!(cpq.pop(), Some(c));

    println!("OK");
}