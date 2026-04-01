//! Trading-specific metric helpers — utility functions for annualising,
//! normalising, and computing common quantitative finance performance
//! measures from a series of returns.
//!
//! These helpers are used when processing responses from external trading APIs
//! (e.g. computing a Sharpe ratio from daily returns before passing it into
//! the fitness engine).

/// Trading days in a standard year (US equity markets).
const TRADING_DAYS_PER_YEAR: f64 = 252.0;

/// Annualise a daily Sharpe ratio.
///
/// `annualised = daily_sharpe * sqrt(252)`
pub fn annualize_sharpe(daily_sharpe: f64) -> f64 {
    daily_sharpe * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Annualise a daily Sortino ratio.
///
/// `annualised = daily_sortino * sqrt(252)`
pub fn annualize_sortino(daily_sortino: f64) -> f64 {
    daily_sortino * TRADING_DAYS_PER_YEAR.sqrt()
}

/// Compute the maximum drawdown from a series of period returns.
///
/// Returns a value in `[0.0, 1.0]` representing the largest peak-to-trough
/// decline measured on the cumulative equity curve. A value of 0 means no
/// drawdown (e.g. all positive returns); a value of 1 means a complete wipeout.
///
/// If the input is empty, returns `0.0`.
pub fn compute_max_drawdown(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let mut peak = 1.0_f64;
    let mut equity = 1.0_f64;
    let mut max_dd = 0.0_f64;

    for &r in returns {
        equity *= 1.0 + r;
        if equity > peak {
            peak = equity;
        }
        let drawdown = (peak - equity) / peak;
        if drawdown > max_dd {
            max_dd = drawdown;
        }
    }

    max_dd
}

/// Compute the win rate — the percentage of returns that are strictly positive.
///
/// Returns a value in `[0.0, 1.0]`. An empty slice yields `0.0`.
pub fn compute_win_rate(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    let wins = returns.iter().filter(|&&r| r > 0.0).count();
    wins as f64 / returns.len() as f64
}

/// Compute the total (cumulative) return from a series of period returns.
///
/// `total_return = product(1 + r_i) - 1`
///
/// An empty slice yields `0.0`.
pub fn compute_total_return(returns: &[f64]) -> f64 {
    if returns.is_empty() {
        return 0.0;
    }

    returns.iter().fold(1.0_f64, |acc, &r| acc * (1.0 + r)) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-10;

    #[test]
    fn test_annualize_sharpe() {
        let daily = 0.1;
        let annualised = annualize_sharpe(daily);
        let expected = 0.1 * (252.0_f64).sqrt();
        assert!((annualised - expected).abs() < EPSILON);
    }

    #[test]
    fn test_annualize_sortino() {
        let daily = 0.2;
        let annualised = annualize_sortino(daily);
        let expected = 0.2 * (252.0_f64).sqrt();
        assert!((annualised - expected).abs() < EPSILON);
    }

    #[test]
    fn test_max_drawdown_no_data() {
        assert_eq!(compute_max_drawdown(&[]), 0.0);
    }

    #[test]
    fn test_max_drawdown_only_gains() {
        let returns = vec![0.01, 0.02, 0.03];
        assert_eq!(compute_max_drawdown(&returns), 0.0);
    }

    #[test]
    fn test_max_drawdown_simple() {
        // Equity: 1.0 -> 1.1 -> 0.99 -> 1.089
        // Peak at 1.1, trough at 0.99 => dd = (1.1 - 0.99) / 1.1 = 0.1
        let returns = vec![0.1, -0.1, 0.1];
        let dd = compute_max_drawdown(&returns);
        assert!((dd - 0.1).abs() < 1e-9);
    }

    #[test]
    fn test_max_drawdown_total_loss() {
        let returns = vec![-1.0];
        let dd = compute_max_drawdown(&returns);
        assert!((dd - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_win_rate_empty() {
        assert_eq!(compute_win_rate(&[]), 0.0);
    }

    #[test]
    fn test_win_rate_all_wins() {
        let returns = vec![0.01, 0.02, 0.005];
        assert!((compute_win_rate(&returns) - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_win_rate_mixed() {
        let returns = vec![0.01, -0.02, 0.03, -0.01];
        assert!((compute_win_rate(&returns) - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_win_rate_zero_not_a_win() {
        let returns = vec![0.0, 0.0, 0.01];
        // Only 1 out of 3 is strictly positive.
        assert!((compute_win_rate(&returns) - 1.0 / 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_total_return_empty() {
        assert_eq!(compute_total_return(&[]), 0.0);
    }

    #[test]
    fn test_total_return_single() {
        let returns = vec![0.1];
        assert!((compute_total_return(&returns) - 0.1).abs() < EPSILON);
    }

    #[test]
    fn test_total_return_compound() {
        // (1 + 0.1) * (1 + 0.2) - 1 = 1.32 - 1 = 0.32
        let returns = vec![0.1, 0.2];
        assert!((compute_total_return(&returns) - 0.32).abs() < EPSILON);
    }

    #[test]
    fn test_total_return_loss() {
        // (1 - 0.5) * (1 + 1.0) - 1 = 1.0 - 1 = 0.0
        let returns = vec![-0.5, 1.0];
        assert!((compute_total_return(&returns) - 0.0).abs() < EPSILON);
    }
}
