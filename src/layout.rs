#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grid {
    pub rows: usize,
    pub cols: usize,
}

pub fn grid_for_count(count: usize) -> Grid {
    match count {
        0 | 1 => Grid { rows: 1, cols: 1 },
        2 => Grid { rows: 1, cols: 2 },
        3 | 4 => Grid { rows: 2, cols: 2 },
        5 | 6 => Grid { rows: 2, cols: 3 },
        n => {
            let cols = (n as f64).sqrt().ceil() as usize;
            let rows = n.div_ceil(cols);
            Grid { rows, cols }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Grid, grid_for_count};

    #[test]
    fn uses_expected_fixed_layouts() {
        assert_eq!(grid_for_count(1), Grid { rows: 1, cols: 1 });
        assert_eq!(grid_for_count(2), Grid { rows: 1, cols: 2 });
        assert_eq!(grid_for_count(3), Grid { rows: 2, cols: 2 });
        assert_eq!(grid_for_count(4), Grid { rows: 2, cols: 2 });
        assert_eq!(grid_for_count(5), Grid { rows: 2, cols: 3 });
        assert_eq!(grid_for_count(6), Grid { rows: 2, cols: 3 });
    }

    #[test]
    fn uses_roughly_square_layout_for_larger_counts() {
        assert_eq!(grid_for_count(7), Grid { rows: 3, cols: 3 });
        assert_eq!(grid_for_count(10), Grid { rows: 3, cols: 4 });
    }
}
