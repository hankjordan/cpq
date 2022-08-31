use cpq::*;

#[derive(Hash, Clone, Copy, PartialEq, Eq, Debug)]
pub struct Location {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

fn main() {
    let cpq: ConcurrentPriorityQueue<Location, u32> = ConcurrentPriorityQueue::new();
    println!("Peek: {:?}", cpq.peek());
}