use std::collections::VecDeque;

pub struct Blacklist<T>
where
    T: PartialEq,
{
    already_checked: Vec<T>,
    queue: VecDeque<T>,
}

impl<T: PartialEq> Blacklist<T> {
    pub fn new() -> Blacklist<T> {
        Blacklist {
            already_checked: Vec::new(),
            queue: VecDeque::new(),
        }
    }
    pub fn add_to_queue(&mut self, item: T) {
        if !self.already_checked.contains(&item) {
            self.queue.push_back(item)
        }
    }
    pub fn get_next(&self) -> Option<&T> {
        self.queue.front()
    }
    pub fn is_queue_empty(&self) -> bool {
        self.queue.is_empty()
    }
    pub fn pop_next(&mut self) {
        if let Some(item) = self.queue.pop_front() {
            self.already_checked.push(item);
        }
    }
}
