//! The vector segment: a port of chromadb's fork of hnswlib (`HierarchicalNSW` with its
//! persistent layout of `header.bin`, `data_level0.bin`, `length.bin` and
//! `link_lists.bin`), plus the `index_metadata.pickle` id map chromadb keeps next to it.

use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap, HashMap, HashSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::StoreError;

pub const PERSISTENCE_VERSION: i32 = 1;
pub const DEFAULT_MAX_ELEMENTS: usize = 100;
const DELETE_MARK: u8 = 0x01;
const NONE: u32 = u32::MAX;
pub const METADATA_FILE: &str = "index_metadata.pickle";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Space {
    L2,
    Ip,
    Cosine,
}

impl Space {
    pub fn parse(name: &str) -> Space {
        match name {
            "ip" => Space::Ip,
            "cosine" => Space::Cosine,
            _ => Space::L2,
        }
    }

    pub fn distance(self, a: &[f32], b: &[f32]) -> f32 {
        match self {
            Space::L2 => {
                let mut s = 0f32;
                for (x, y) in a.iter().zip(b) {
                    let d = x - y;
                    s += d * d;
                }
                s
            }
            Space::Ip | Space::Cosine => {
                let mut s = 0f32;
                for (x, y) in a.iter().zip(b) {
                    s += x * y;
                }
                1.0 - s
            }
        }
    }
}

/// `std::minstd_rand0`, libstdc++'s `default_random_engine`.
#[derive(Debug, Clone)]
struct MinStd(u64);

impl MinStd {
    const M: u64 = 2_147_483_647;

    fn new(seed: u64) -> Self {
        let s = seed % Self::M;
        MinStd(if s == 0 { 1 } else { s })
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0 * 16807 % Self::M;
        self.0
    }

    /// `uniform_real_distribution<double>(0, 1)`.
    fn uniform_f64(&mut self) -> f64 {
        let r = (Self::M - 1) as f64;
        let a = (self.next() - 1) as f64;
        let b = (self.next() - 1) as f64;
        let v = (a + b * r) / (r * r);
        if v >= 1.0 {
            1.0 - f64::EPSILON / 2.0
        } else {
            v
        }
    }

    /// `uniform_real_distribution<float>(0, 1)`.
    fn uniform_f32(&mut self) -> f32 {
        let r = (Self::M - 1) as f32;
        let v = (self.next() - 1) as f32 / r;
        if v >= 1.0 {
            1.0 - f32::EPSILON / 2.0
        } else {
            v
        }
    }
}

/// A float ordered for the heaps.
#[derive(Debug, Clone, Copy, PartialEq)]
struct D(f32);

impl Eq for D {}

impl PartialOrd for D {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for D {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// A max-heap of (distance, internal id).
type Heap = BinaryHeap<(D, u32)>;

pub struct Hnsw {
    pub dim: usize,
    pub space: Space,
    normalize: bool,
    pub max_elements: usize,
    pub cur_count: usize,
    m: usize,
    max_m: usize,
    max_m0: usize,
    ef_construction: usize,
    pub ef: usize,
    mult: f64,
    size_data_per_element: usize,
    size_links_per_element: usize,
    offset_data: usize,
    label_offset: usize,
    data0: Vec<u8>,
    length: Vec<u8>,
    links: Vec<Option<Vec<u8>>>,
    levels: Vec<i32>,
    label_lookup: HashMap<u64, u32>,
    enterpoint: u32,
    maxlevel: i32,
    pub num_deleted: usize,
    rng: MinStd,
    update_rng: MinStd,
    dirty: BTreeSet<u32>,
}

fn rd_u32(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn wr_u32(b: &mut [u8], at: usize, v: u32) {
    b[at..at + 4].copy_from_slice(&v.to_le_bytes());
}

impl Hnsw {
    pub fn new(
        dim: usize,
        space: Space,
        max_elements: usize,
        m: usize,
        ef_construction: usize,
        seed: u64,
    ) -> Hnsw {
        let max_m0 = m * 2;
        let data_size = dim * 4;
        let size_links_level0 = max_m0 * 4 + 4;
        let size_data_per_element = size_links_level0 + data_size + 8;
        let mult = 1.0 / (m as f64).ln();
        Hnsw {
            dim,
            space,
            normalize: space == Space::Cosine,
            max_elements,
            cur_count: 0,
            m,
            max_m: m,
            max_m0,
            ef_construction: ef_construction.max(m),
            ef: 10,
            mult,
            size_data_per_element,
            size_links_per_element: m * 4 + 4,
            offset_data: size_links_level0,
            label_offset: size_links_level0 + data_size,
            data0: vec![0; max_elements * size_data_per_element],
            length: vec![0; max_elements * 4],
            links: vec![None; max_elements],
            levels: vec![0; max_elements],
            label_lookup: HashMap::new(),
            enterpoint: NONE,
            maxlevel: -1,
            num_deleted: 0,
            rng: MinStd::new(seed),
            update_rng: MinStd::new(seed + 1),
            dirty: BTreeSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.cur_count - self.num_deleted
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn header(&self) -> Vec<u8> {
        let mut h = Vec::with_capacity(100);
        h.extend(PERSISTENCE_VERSION.to_le_bytes());
        for v in [
            0usize,
            self.max_elements,
            self.cur_count,
            self.size_data_per_element,
            self.label_offset,
            self.offset_data,
        ] {
            h.extend((v as u64).to_le_bytes());
        }
        h.extend(self.maxlevel.to_le_bytes());
        h.extend(self.enterpoint.to_le_bytes());
        for v in [self.max_m, self.max_m0, self.m] {
            h.extend((v as u64).to_le_bytes());
        }
        h.extend(self.mult.to_le_bytes());
        h.extend((self.ef_construction as u64).to_le_bytes());
        h
    }

    fn link_lists_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        for i in 0..self.cur_count {
            let size = if self.levels[i] > 0 {
                self.size_links_per_element * self.levels[i] as usize
            } else {
                0
            };
            out.extend((size as u32).to_le_bytes());
            if size > 0
                && let Some(l) = &self.links[i]
            {
                out.extend(&l[..size]);
            }
        }
        out
    }

    /// `initPersistentIndex`: the four files of an index, written whole.
    pub fn write_files(&mut self, dir: &Path) -> Result<(), StoreError> {
        fs::write(dir.join("header.bin"), self.header())?;
        fs::write(dir.join("data_level0.bin"), &self.data0)?;
        fs::write(dir.join("length.bin"), &self.length)?;
        fs::write(dir.join("link_lists.bin"), self.link_lists_bytes())?;
        self.dirty.clear();
        Ok(())
    }

    /// `persistDirty`: the files hold the whole index afterwards.
    pub fn persist(&mut self, dir: &Path) -> Result<(), StoreError> {
        if self.dirty.is_empty() {
            return Ok(());
        }
        self.write_files(dir)
    }

    /// `loadPersistedIndex`.
    pub fn load(
        dir: &Path,
        dim: usize,
        space: Space,
        ef_search: usize,
    ) -> Result<Hnsw, StoreError> {
        let header = fs::read(dir.join("header.bin"))?;
        if header.len() < 100 {
            return Err(StoreError::new("Error loading hnsw index"));
        }
        let i32_at = |at: usize| {
            i32::from_le_bytes([header[at], header[at + 1], header[at + 2], header[at + 3]])
        };
        let u64_at = |at: usize| {
            let mut b = [0u8; 8];
            b.copy_from_slice(&header[at..at + 8]);
            u64::from_le_bytes(b) as usize
        };
        if i32_at(0) != PERSISTENCE_VERSION {
            return Err(StoreError::new(
                "Cannot read persisted index: wrong persistence version",
            ));
        }
        let max_elements = u64_at(12);
        let cur_count = u64_at(20);
        let size_data_per_element = u64_at(28);
        let label_offset = u64_at(36);
        let offset_data = u64_at(44);
        let maxlevel = i32_at(52);
        let enterpoint = rd_u32(&header, 56);
        let max_m = u64_at(60);
        let max_m0 = u64_at(68);
        let m = u64_at(76);
        let mut f = [0u8; 8];
        f.copy_from_slice(&header[84..92]);
        let mult = f64::from_le_bytes(f);
        let ef_construction = u64_at(92);
        let mut data0 = fs::read(dir.join("data_level0.bin"))?;
        data0.resize(max_elements * size_data_per_element, 0);
        let mut length = fs::read(dir.join("length.bin"))?;
        length.resize(max_elements * 4, 0);
        let link_bytes = fs::read(dir.join("link_lists.bin"))?;
        let size_links_per_element = max_m * 4 + 4;
        let mut idx = Hnsw {
            dim,
            space,
            normalize: space == Space::Cosine,
            max_elements,
            cur_count,
            m,
            max_m,
            max_m0,
            ef_construction,
            ef: ef_search,
            mult,
            size_data_per_element,
            size_links_per_element,
            offset_data,
            label_offset,
            data0,
            length,
            links: vec![None; max_elements],
            levels: vec![0; max_elements],
            label_lookup: HashMap::new(),
            enterpoint,
            maxlevel,
            num_deleted: 0,
            rng: MinStd::new(0),
            update_rng: MinStd::new(1),
            dirty: BTreeSet::new(),
        };
        let mut at = 0;
        for i in 0..cur_count {
            idx.label_lookup.insert(idx.label(i as u32), i as u32);
            if at + 4 > link_bytes.len() {
                return Err(StoreError::new("Error loading hnsw index"));
            }
            let size = rd_u32(&link_bytes, at) as usize;
            at += 4;
            if size > 0 {
                if at + size > link_bytes.len() {
                    return Err(StoreError::new("Error loading hnsw index"));
                }
                idx.levels[i] = (size / size_links_per_element) as i32;
                let mut l = link_bytes[at..at + size].to_vec();
                l.push(0);
                idx.links[i] = Some(l);
                at += size;
            }
            if idx.is_deleted(i as u32) {
                idx.num_deleted += 1;
            }
        }
        Ok(idx)
    }

    pub fn resize(&mut self, new_max: usize) -> Result<(), StoreError> {
        if new_max < self.cur_count {
            return Err(StoreError::new(
                "Cannot resize, max element is less than the current number of elements",
            ));
        }
        self.data0.resize(new_max * self.size_data_per_element, 0);
        self.length.resize(new_max * 4, 0);
        self.links.resize(new_max, None);
        self.levels.resize(new_max, 0);
        self.max_elements = new_max;
        Ok(())
    }

    fn label(&self, i: u32) -> u64 {
        let at = i as usize * self.size_data_per_element + self.label_offset;
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.data0[at..at + 8]);
        u64::from_le_bytes(b)
    }

    fn vector(&self, i: u32) -> Vec<f32> {
        let at = i as usize * self.size_data_per_element + self.offset_data;
        self.data0[at..at + self.dim * 4]
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect()
    }

    fn dist_to(&self, q: &[f32], i: u32) -> f32 {
        let at = i as usize * self.size_data_per_element + self.offset_data;
        let bytes = &self.data0[at..at + self.dim * 4];
        let v: Vec<f32> = bytes
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        self.space.distance(q, &v)
    }

    fn dist(&self, a: u32, b: u32) -> f32 {
        let va = self.vector(a);
        self.dist_to(&va, b)
    }

    fn ll0_at(&self, i: u32) -> usize {
        i as usize * self.size_data_per_element
    }

    fn is_deleted(&self, i: u32) -> bool {
        self.data0[self.ll0_at(i) + 2] & DELETE_MARK != 0
    }

    /// The neighbours of `i` at `level`.
    fn neighbours(&self, i: u32, level: i32) -> Vec<u32> {
        let (buf, at): (&[u8], usize) = if level == 0 {
            (&self.data0, self.ll0_at(i))
        } else {
            match &self.links[i as usize] {
                Some(l) => (l, (level as usize - 1) * self.size_links_per_element),
                None => return Vec::new(),
            }
        };
        let count = u16::from_le_bytes([buf[at], buf[at + 1]]) as usize;
        (0..count).map(|j| rd_u32(buf, at + 4 + j * 4)).collect()
    }

    fn set_neighbours(&mut self, i: u32, level: i32, list: &[u32]) {
        let spl = self.size_links_per_element;
        let (buf, at): (&mut [u8], usize) = if level == 0 {
            let at = self.ll0_at(i);
            (&mut self.data0, at)
        } else {
            let l = self.links[i as usize].as_mut().expect("link list");
            (l.as_mut_slice(), (level as usize - 1) * spl)
        };
        let c = (list.len() as u16).to_le_bytes();
        buf[at] = c[0];
        buf[at + 1] = c[1];
        for (j, v) in list.iter().enumerate() {
            wr_u32(buf, at + 4 + j * 4, *v);
        }
    }

    /// `searchBaseLayer`, used while building.
    fn search_base_layer(&self, ep: u32, q: &[f32], layer: i32) -> Heap {
        let mut visited = HashSet::new();
        let mut top: Heap = BinaryHeap::new();
        let mut cand: BinaryHeap<(D, u32)> = BinaryHeap::new();
        let mut lower;
        if !self.is_deleted(ep) {
            let d = self.dist_to(q, ep);
            top.push((D(d), ep));
            lower = d;
            cand.push((D(-d), ep));
        } else {
            lower = f32::MAX;
            cand.push((D(-lower), ep));
        }
        visited.insert(ep);
        while let Some(&(D(negd), cur)) = cand.peek() {
            if -negd > lower && top.len() == self.ef_construction {
                break;
            }
            cand.pop();
            for c in self.neighbours(cur, layer) {
                if !visited.insert(c) {
                    continue;
                }
                let d = self.dist_to(q, c);
                if top.len() < self.ef_construction || lower > d {
                    cand.push((D(-d), c));
                    if !self.is_deleted(c) {
                        top.push((D(d), c));
                    }
                    if top.len() > self.ef_construction {
                        top.pop();
                    }
                    if let Some(&(D(t), _)) = top.peek() {
                        lower = t;
                    }
                }
            }
        }
        top
    }

    /// `searchBaseLayerST`, used by queries.
    fn search_base_layer_st(
        &self,
        ep: u32,
        q: &[f32],
        ef: usize,
        allowed: &dyn Fn(u64) -> bool,
        filtered: bool,
    ) -> Heap {
        let has_deletions = self.num_deleted > 0;
        let mut visited = HashSet::new();
        let mut top: Heap = BinaryHeap::new();
        let mut cand: BinaryHeap<(D, u32)> = BinaryHeap::new();
        let mut lower;
        if (!has_deletions || !self.is_deleted(ep)) && allowed(self.label(ep)) {
            let d = self.dist_to(q, ep);
            lower = d;
            top.push((D(d), ep));
            cand.push((D(-d), ep));
        } else {
            lower = f32::MAX;
            cand.push((D(-lower), ep));
        }
        visited.insert(ep);
        while let Some(&(D(negd), cur)) = cand.peek() {
            if -negd > lower && (top.len() == ef || (!filtered && !has_deletions)) {
                break;
            }
            cand.pop();
            for c in self.neighbours(cur, 0) {
                if !visited.insert(c) {
                    continue;
                }
                let d = self.dist_to(q, c);
                if top.len() < ef || lower > d {
                    cand.push((D(-d), c));
                    if (!has_deletions || !self.is_deleted(c)) && allowed(self.label(c)) {
                        top.push((D(d), c));
                    }
                    if top.len() > ef {
                        top.pop();
                    }
                    if let Some(&(D(t), _)) = top.peek() {
                        lower = t;
                    }
                }
            }
        }
        top
    }

    /// `getNeighborsByHeuristic2`.
    fn heuristic(&self, top: &mut Heap, m: usize) {
        if top.len() < m {
            return;
        }
        let mut closest: BinaryHeap<(D, u32)> = BinaryHeap::new();
        while let Some((D(d), i)) = top.pop() {
            closest.push((D(-d), i));
        }
        let mut ret: Vec<(f32, u32)> = Vec::new();
        while let Some((D(negd), i)) = closest.pop() {
            if ret.len() >= m {
                break;
            }
            let dq = -negd;
            let good = ret.iter().all(|&(_, j)| self.dist(j, i) >= dq);
            if good {
                ret.push((negd, i));
            }
        }
        for (negd, i) in ret {
            top.push((D(-negd), i));
        }
    }

    fn mutually_connect(
        &mut self,
        cur: u32,
        mut top: Heap,
        level: i32,
        is_update: bool,
    ) -> Result<u32, StoreError> {
        self.dirty.insert(cur);
        let mcur = if level > 0 { self.max_m } else { self.max_m0 };
        self.heuristic(&mut top, self.m);
        let mut selected = Vec::new();
        while let Some((_, i)) = top.pop() {
            selected.push(i);
        }
        let next = *selected
            .last()
            .ok_or_else(|| StoreError::new("Error applying log chunk"))?;
        self.set_neighbours(cur, level, &selected);
        for &s in &selected {
            self.dirty.insert(s);
            let others = self.neighbours(s, level);
            if others.len() > mcur || s == cur {
                return Err(StoreError::new("Error applying log chunk"));
            }
            if is_update && others.contains(&cur) {
                continue;
            }
            if others.len() < mcur {
                let mut l = others;
                l.push(cur);
                self.set_neighbours(s, level, &l);
            } else {
                let mut cands: Heap = BinaryHeap::new();
                cands.push((D(self.dist(cur, s)), cur));
                for &o in &others {
                    cands.push((D(self.dist(o, s)), o));
                }
                self.heuristic(&mut cands, mcur);
                let mut l = Vec::new();
                while let Some((_, i)) = cands.pop() {
                    l.push(i);
                }
                self.set_neighbours(s, level, &l);
            }
        }
        Ok(next)
    }

    fn normalized(&self, v: &[f32]) -> (Vec<f32>, f32) {
        if !self.normalize {
            return (v.to_vec(), 1.0);
        }
        let norm: f32 = v.iter().map(|x| x * x).sum();
        let length = norm.sqrt();
        let inv = 1.0 / (length + 1e-30);
        (v.iter().map(|x| x * inv).collect(), length)
    }

    fn write_vector(&mut self, i: u32, v: &[f32], length: f32) {
        let at = i as usize * self.size_data_per_element + self.offset_data;
        for (j, x) in v.iter().enumerate() {
            self.data0[at + j * 4..at + j * 4 + 4].copy_from_slice(&x.to_le_bytes());
        }
        if self.normalize {
            self.length[i as usize * 4..i as usize * 4 + 4].copy_from_slice(&length.to_le_bytes());
        }
    }

    /// `addPoint`: adds the vector, or updates it when the label exists.
    pub fn add(&mut self, label: u64, v: &[f32]) -> Result<(), StoreError> {
        if v.len() != self.dim {
            return Err(StoreError::new("Error applying log chunk"));
        }
        if let Some(&existing) = self.label_lookup.get(&label) {
            if self.is_deleted(existing) {
                self.unmark_deleted(existing);
            }
            return self.update_point(v, existing);
        }
        if self.cur_count >= self.max_elements {
            return Err(StoreError::new(
                "The number of elements exceeds the specified limit",
            ));
        }
        let cur = self.cur_count as u32;
        self.cur_count += 1;
        self.label_lookup.insert(label, cur);
        let u = self.rng.uniform_f64();
        let curlevel = (-(u.ln()) * self.mult) as i32;
        self.levels[cur as usize] = curlevel;
        let maxlevelcopy = self.maxlevel;
        let mut curr_obj = self.enterpoint;
        let ep_copy = self.enterpoint;
        let at = self.ll0_at(cur);
        let sz = self.size_data_per_element;
        self.data0[at..at + sz].fill(0);
        let (nv, length) = self.normalized(v);
        if self.normalize {
            self.length[cur as usize * 4..cur as usize * 4 + 4]
                .copy_from_slice(&length.to_le_bytes());
        }
        let lab = cur as usize * sz + self.label_offset;
        self.data0[lab..lab + 8].copy_from_slice(&label.to_le_bytes());
        self.write_vector(cur, &nv, length);
        if curlevel > 0 {
            self.links[cur as usize] =
                Some(vec![0; self.size_links_per_element * curlevel as usize + 1]);
        }
        if curr_obj != NONE {
            if curlevel < maxlevelcopy {
                let mut curdist = self.dist_to(&nv, curr_obj);
                let mut level = maxlevelcopy;
                while level > curlevel {
                    let mut changed = true;
                    while changed {
                        changed = false;
                        for c in self.neighbours(curr_obj, level) {
                            if c as usize > self.max_elements {
                                return Err(StoreError::new("cand error"));
                            }
                            let d = self.dist_to(&nv, c);
                            if d < curdist {
                                curdist = d;
                                curr_obj = c;
                                changed = true;
                            }
                        }
                    }
                    level -= 1;
                }
            }
            let ep_deleted = self.is_deleted(ep_copy);
            let mut level = curlevel.min(maxlevelcopy);
            while level >= 0 {
                let mut top = self.search_base_layer(curr_obj, &nv, level);
                if ep_deleted {
                    top.push((D(self.dist_to(&nv, ep_copy)), ep_copy));
                    if top.len() > self.ef_construction {
                        top.pop();
                    }
                }
                curr_obj = self.mutually_connect(cur, top, level, false)?;
                level -= 1;
            }
        } else {
            self.enterpoint = 0;
            self.maxlevel = curlevel;
            self.dirty.insert(cur);
        }
        if curlevel > maxlevelcopy {
            self.enterpoint = cur;
            self.maxlevel = curlevel;
        }
        Ok(())
    }

    fn update_point(&mut self, v: &[f32], id: u32) -> Result<(), StoreError> {
        let (nv, length) = self.normalized(v);
        self.write_vector(id, &nv, length);
        self.dirty.insert(id);
        let max_level_copy = self.maxlevel;
        let ep_copy = self.enterpoint;
        if ep_copy == id && self.cur_count == 1 {
            return Ok(());
        }
        let elem_level = self.levels[id as usize];
        for layer in 0..=elem_level {
            let one_hop = self.neighbours(id, layer);
            if one_hop.is_empty() {
                continue;
            }
            let mut s_cand: Vec<u32> = vec![id];
            let mut s_neigh: Vec<u32> = Vec::new();
            for &h in &one_hop {
                if !s_cand.contains(&h) {
                    s_cand.push(h);
                }
                if self.update_rng.uniform_f32() > 1.0 {
                    continue;
                }
                if !s_neigh.contains(&h) {
                    s_neigh.push(h);
                }
                for h2 in self.neighbours(h, layer) {
                    if !s_cand.contains(&h2) {
                        s_cand.push(h2);
                    }
                }
            }
            for &neigh in &s_neigh {
                let size = if s_cand.contains(&neigh) {
                    s_cand.len() - 1
                } else {
                    s_cand.len()
                };
                let keep = self.ef_construction.min(size);
                let mut cands: Heap = BinaryHeap::new();
                for &c in &s_cand {
                    if c == neigh {
                        continue;
                    }
                    let d = self.dist(neigh, c);
                    if cands.len() < keep {
                        cands.push((D(d), c));
                    } else if let Some(&(D(t), _)) = cands.peek()
                        && d < t
                    {
                        cands.pop();
                        cands.push((D(d), c));
                    }
                }
                let m = if layer == 0 { self.max_m0 } else { self.max_m };
                self.heuristic(&mut cands, m);
                let mut l = Vec::new();
                while let Some((_, i)) = cands.pop() {
                    l.push(i);
                }
                self.set_neighbours(neigh, layer, &l);
            }
        }
        self.repair_connections(&nv, ep_copy, id, elem_level, max_level_copy)
    }

    fn repair_connections(
        &mut self,
        q: &[f32],
        ep: u32,
        id: u32,
        level_of: i32,
        max_level: i32,
    ) -> Result<(), StoreError> {
        let mut curr_obj = ep;
        if level_of < max_level {
            let mut curdist = self.dist_to(q, curr_obj);
            let mut level = max_level;
            while level > level_of {
                let mut changed = true;
                while changed {
                    changed = false;
                    for c in self.neighbours(curr_obj, level) {
                        let d = self.dist_to(q, c);
                        if d < curdist {
                            curdist = d;
                            curr_obj = c;
                            changed = true;
                        }
                    }
                }
                level -= 1;
            }
        }
        if level_of > max_level {
            return Err(StoreError::new(
                "Level of item to be updated cannot be bigger than max level",
            ));
        }
        let mut level = level_of;
        while level >= 0 {
            let top = self.search_base_layer(curr_obj, q, level);
            let mut filtered: Heap = top.into_iter().filter(|&(_, i)| i != id).collect();
            if !filtered.is_empty() {
                if self.is_deleted(ep) {
                    filtered.push((D(self.dist_to(q, ep)), ep));
                    if filtered.len() > self.ef_construction {
                        filtered.pop();
                    }
                }
                curr_obj = self.mutually_connect(id, filtered, level, true)?;
            }
            level -= 1;
        }
        Ok(())
    }

    fn unmark_deleted(&mut self, i: u32) {
        let at = self.ll0_at(i) + 2;
        if self.data0[at] & DELETE_MARK != 0 {
            self.data0[at] &= !DELETE_MARK;
            self.num_deleted -= 1;
        }
    }

    /// `markDelete`.
    pub fn mark_delete(&mut self, label: u64) -> Result<(), StoreError> {
        let Some(&i) = self.label_lookup.get(&label) else {
            return Err(StoreError::new("Label not found"));
        };
        let at = self.ll0_at(i) + 2;
        if self.data0[at] & DELETE_MARK != 0 {
            return Err(StoreError::new(
                "The requested to delete element is already deleted",
            ));
        }
        self.data0[at] |= DELETE_MARK;
        self.num_deleted += 1;
        self.dirty.insert(i);
        Ok(())
    }

    /// `getDataByLabel`: the stored vector (scaled back for cosine).
    pub fn get(&self, label: u64) -> Option<Vec<f32>> {
        let &i = self.label_lookup.get(&label)?;
        if self.is_deleted(i) {
            return None;
        }
        let v = self.vector(i);
        if !self.normalize {
            return Some(v);
        }
        let at = i as usize * 4;
        let length = f32::from_le_bytes([
            self.length[at],
            self.length[at + 1],
            self.length[at + 2],
            self.length[at + 3],
        ]);
        Some(v.iter().map(|x| x * length).collect())
    }

    /// Live and deleted labels.
    pub fn labels(&self) -> (Vec<u64>, Vec<u64>) {
        let mut live = Vec::new();
        let mut deleted = Vec::new();
        for (&l, &i) in &self.label_lookup {
            if self.is_deleted(i) {
                deleted.push(l)
            } else {
                live.push(l)
            }
        }
        (live, deleted)
    }

    /// `searchKnn` with an allow list (empty: everything): (distance, label), nearest first.
    pub fn search(&self, q: &[f32], k: usize, allowed: &[u64]) -> Vec<(f32, u64)> {
        if self.cur_count == 0 {
            return Vec::new();
        }
        let q = self.normalized(q).0;
        let allow: HashSet<u64> = allowed.iter().copied().collect();
        let is_allowed = |l: u64| allow.is_empty() || allow.contains(&l);
        let mut curr_obj = self.enterpoint;
        let mut curdist = self.dist_to(&q, curr_obj);
        let mut level = self.maxlevel;
        while level > 0 {
            let mut changed = true;
            while changed {
                changed = false;
                for c in self.neighbours(curr_obj, level) {
                    let d = self.dist_to(&q, c);
                    if d < curdist {
                        curdist = d;
                        curr_obj = c;
                        changed = true;
                    }
                }
            }
            level -= 1;
        }
        let mut top = self.search_base_layer_st(curr_obj, &q, self.ef.max(k), &is_allowed, true);
        while top.len() > k {
            top.pop();
        }
        let mut out: Vec<(f32, u64)> = top
            .into_iter()
            .map(|(D(d), i)| (d, self.label(i)))
            .collect();
        // `knnQuery` pops the (distance, label) queue from the back: ties by label.
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        out
    }
}

/// `index_metadata.pickle`: the user ids of the labels.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct IdMap {
    pub dimensionality: Option<usize>,
    pub total_elements_added: u32,
    #[serde(default)]
    pub max_seq_id: Option<u64>,
    pub id_to_label: HashMap<String, u32>,
    pub label_to_id: HashMap<u32, String>,
    pub id_to_seq_id: HashMap<String, u32>,
}

impl IdMap {
    pub fn read(path: &Path) -> Result<IdMap, StoreError> {
        let bytes = fs::read(path)?;
        serde_pickle::from_slice(&bytes, serde_pickle::DeOptions::new())
            .map_err(|e| StoreError::new(format!("Error deserializing pickle file: {e}")))
    }

    pub fn write(&self, path: &Path) -> Result<(), StoreError> {
        let bytes = serde_pickle::to_vec(self, serde_pickle::SerOptions::new())
            .map_err(|e| StoreError::new(format!("Error serializing pickle file: {e}")))?;
        fs::write(path, bytes)?;
        Ok(())
    }
}
