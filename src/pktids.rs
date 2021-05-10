use std::ops::RangeInclusive;

use crate::protocol::PktId;

pub(crate) struct PktIds {
    start: usize,
    ranges: Vec<Node>,
}

enum Node {
    Vacant,
    Taken {
        range: RangeInclusive<u16>,
        next: usize,
    },
}

impl PktIds {
    pub(crate) fn new() -> Self {
        Self::new_with_max(PktId::new(u16::MAX).unwrap())
    }

    pub(crate) fn new_with_max(max: PktId) -> Self {
        let max = max.get();
        let node = Node::Taken {
            range: 1..=max,
            next: 0,
        };
        Self {
            ranges: vec![Node::Vacant, node],
            start: 1,
        }
    }

    pub(crate) fn next_id(&mut self) -> Option<PktId> {
        // take node out of vec
        let old_start = self.start;
        let mut node = match self.ranges.get_mut(old_start) {
            None => return None,
            Some(v) => std::mem::replace(v, Node::Vacant),
        };

        let (result, new_node) = match &mut node {
            Node::Vacant => (None, None),
            Node::Taken {
                ref mut range,
                next,
            } => {
                let start = *range.start();
                let end = *range.end();
                let mut pktid = Some(start);
                let mut new_node = None;

                if start >= end {
                    // this range is exhausted
                    if let Some(Node::Vacant) = self.ranges.get(*next) {
                        // if we have no next nodes and all pktids are in use
                        // just set this node's range as exhausted
                        *range = (start + 1)..=end;
                        if start != end {
                            pktid = None;
                        }
                    } else {
                        // if we have next node - set this node as vacant and start using next node
                        new_node = Some(Node::Vacant);
                        self.start = *next;
                    }
                } else {
                    *range = (start + 1)..=end;
                }
                (pktid, new_node)
            }
        };

        // put node back in vec
        match self.ranges.get_mut(old_start) {
            Some(v) => {
                *v = new_node.unwrap_or(node);
            }
            None => {}
        }

        match result {
            Some(pktid) => PktId::new(pktid),
            None => None,
        }
    }

    pub(crate) fn return_id(&mut self, id: PktId) {
        let id = id.get();
        let mut prev_idx = None;
        let mut next_idx = self.start;
        while let Some(Node::Taken { range, next }) = self.ranges.get(next_idx) {
            if *range.start() > id {
                break;
            } else {
                prev_idx = Some(next_idx);
                next_idx = *next;
            }
        }

        match prev_idx {
            // insert to list head
            None => {
                // maybe current list head's lower bound is just larger by 1 than our id
                // then just decrease it by 1, no insertion needed
                if self.maybe_extend_next_node(next_idx, id) {
                    return;
                }

                // we need to insert new node:
                // 1. find vacant node (or add it)
                let vacant_idx = match self.ranges[1..]
                    .iter()
                    .position(|node| matches!(node, Node::Vacant))
                {
                    Some(v) => v + 1,
                    None => {
                        self.ranges.push(Node::Vacant);
                        self.ranges.len() - 1
                    }
                };
                // 2. insert the node
                self.ranges[vacant_idx] = Node::Taken {
                    range: id..=id,
                    next: self.start,
                };
                // 3. update start of the list
                self.start = vacant_idx;
            }
            // insert new node between prev_idx and idx
            Some(prev_idx) => {
                // maybe prev node upper bound is just smaller by 1 than our id
                // then just increase it by 1, no insertion needed
                if self.maybe_extend_prev_node(prev_idx, id) {
                    return;
                }

                // maybe next node lower bound is just larger by 1 than our id
                // then just decrease it by 1, no insertion needed
                if self.maybe_extend_next_node(next_idx, id) {
                    return;
                }

                // no way we could extend already existing ranges, insert new node between prev_idx and next_idx
                // 1. find vacant node (or add it)
                let vacant_idx = match self.ranges[1..]
                    .iter()
                    .position(|node| matches!(node, Node::Vacant))
                {
                    Some(v) => v + 1,
                    None => {
                        self.ranges.push(Node::Vacant);
                        self.ranges.len() - 1
                    }
                };
                // 2. insert the node
                self.ranges[vacant_idx] = Node::Taken {
                    range: id..=id,
                    next: next_idx,
                };
                // 3. update prev node to point to new node
                match self.ranges.get_mut(prev_idx) {
                    None => unreachable!(),
                    Some(Node::Vacant) => unreachable!(),
                    Some(Node::Taken { next, .. }) => {
                        *next = vacant_idx;
                    }
                }
            }
        }
    }

    fn maybe_extend_next_node(&mut self, next_node: usize, id: u16) -> bool {
        match self.ranges.get_mut(next_node) {
            None => unreachable!(),
            Some(Node::Vacant) => {}
            Some(Node::Taken { range, .. }) => {
                if range.start() - 1 == id {
                    *range = id..=(*range.end());
                    return true;
                }
            }
        };

        false
    }

    fn maybe_extend_prev_node(&mut self, prev_node: usize, id: u16) -> bool {
        match self.ranges.get_mut(prev_node) {
            None => unreachable!(),
            Some(Node::Vacant) => {}
            Some(Node::Taken { range, .. }) => {
                if range.end() + 1 == id {
                    *range = (*range.start())..=id;
                    // todo: collapse prev and next if they are off by 1
                    return true;
                }
            }
        };

        false
    }
}

#[cfg(test)]
mod tests {
    use super::{PktId, PktIds};

    #[test]
    fn fetch_ids() {
        let mut pktids = PktIds::new();
        assert_eq!(pktids.next_id().map(PktId::get), Some(1));
        assert_eq!(pktids.next_id().map(PktId::get), Some(2));
    }

    #[test]
    fn fetch_ids_exhausted() {
        let mut pktids = PktIds::new_with_max(PktId::new(5).unwrap());
        assert_eq!(pktids.next_id().map(PktId::get), Some(1));
        assert_eq!(pktids.next_id().map(PktId::get), Some(2));
        assert_eq!(pktids.next_id().map(PktId::get), Some(3));
        assert_eq!(pktids.next_id().map(PktId::get), Some(4));
        assert_eq!(pktids.next_id().map(PktId::get), Some(5));
        assert_eq!(pktids.next_id().map(PktId::get), None);
        assert_eq!(pktids.next_id().map(PktId::get), None);
    }

    #[test]
    fn return_ids() {
        let mut pktids = PktIds::new_with_max(PktId::new(12).unwrap());
        for i in 1..10 {
            assert_eq!(pktids.next_id().map(PktId::get), Some(i));
        }
        pktids.return_id(PktId::new(3).unwrap());
        pktids.return_id(PktId::new(4).unwrap());
        assert_eq!(pktids.next_id().map(PktId::get), Some(3));
        assert_eq!(pktids.next_id().map(PktId::get), Some(4));
        pktids.return_id(PktId::new(3).unwrap());
        pktids.return_id(PktId::new(4).unwrap());
        pktids.return_id(PktId::new(1).unwrap());
        assert_eq!(pktids.next_id().map(PktId::get), Some(1));
        assert_eq!(pktids.next_id().map(PktId::get), Some(3));
        assert_eq!(pktids.next_id().map(PktId::get), Some(4));
        assert_eq!(pktids.next_id().map(PktId::get), Some(10));
    }

    #[test]
    fn return_ids2() {
        let mut pktids = PktIds::new_with_max(PktId::new(12).unwrap());
        for i in 1..10 {
            assert_eq!(pktids.next_id().map(PktId::get), Some(i));
        }
        pktids.return_id(PktId::new(1).unwrap());
        pktids.return_id(PktId::new(3).unwrap());
        pktids.return_id(PktId::new(4).unwrap());
        assert_eq!(pktids.next_id().map(PktId::get), Some(1));
        assert_eq!(pktids.next_id().map(PktId::get), Some(3));
        assert_eq!(pktids.next_id().map(PktId::get), Some(4));
        assert_eq!(pktids.next_id().map(PktId::get), Some(10));
    }
}
