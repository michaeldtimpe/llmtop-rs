use std::collections::VecDeque;

pub struct RingBuffer {
    data: VecDeque<f64>,
    capacity: usize,
}

impl RingBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, val: f64) {
        if self.data.len() == self.capacity {
            self.data.pop_front();
        }
        self.data.push_back(val);
    }

    pub fn as_slice_pair(&self) -> (&[f64], &[f64]) {
        self.data.as_slices()
    }

    pub fn to_vec(&self) -> Vec<f64> {
        self.data.iter().copied().collect()
    }

    pub fn last(&self) -> Option<f64> {
        self.data.back().copied()
    }
}

pub struct HistorySet {
    pub total_power: RingBuffer,
    pub gpu_pct: RingBuffer,
    pub cpu_temp: RingBuffer,
    pub gpu_temp: RingBuffer,
}

impl HistorySet {
    pub fn new(capacity: usize) -> Self {
        Self {
            total_power: RingBuffer::new(capacity),
            gpu_pct: RingBuffer::new(capacity),
            cpu_temp: RingBuffer::new(capacity),
            gpu_temp: RingBuffer::new(capacity),
        }
    }
}
