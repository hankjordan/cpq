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
    let cpq: ConcurrentPriorityQueue<Location, u32> = ConcurrentPriorityQueue::new();
    
    cpq.push(Location::new(0, 0, 0), 0);
    cpq.push(Location::new(0, 0, 0), 2);
    cpq.push(Location::new(5, 0, 0), 7);
    cpq.push(Location::new(0, 6, 0), 2);

    println!("POP {:?}", cpq.pop());
}