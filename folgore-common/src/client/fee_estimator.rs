//! Generic Fee estimator for all the folgore backend.
use std::collections::BTreeMap;

use crate::prelude::cln_plugin::error;
use crate::prelude::cln_plugin::errors::PluginError;
use crate::prelude::json::Value;

/// Transaction fee rate in satoshis/vByte.
pub type FeeRate = u64;

#[derive(Clone)]
pub struct FeePriority(pub u16, pub &'static str);

/// Various Fee combination that core lightning is using
pub static FEE_RATES: [FeePriority; 4] = [
    FeePriority(2, "CONSERVATIVE"),
    FeePriority(6, "CONSERVATIVE"),
    FeePriority(12, "CONSERVATIVE"),
    FeePriority(100, "CONSERVATIVE"),
];

pub struct FeeEstimator;

impl FeeEstimator {
    pub fn urgent_fee(fees: &BTreeMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&6).copied()
    }

    pub fn hightest_fee(fees: &BTreeMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&2).copied()
    }

    pub fn normal_fee(fees: &BTreeMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&12).copied()
    }

    pub fn slow_fee(fees: &BTreeMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&100).copied()
    }

    pub fn build_estimate_fees(fees: &BTreeMap<u64, FeeRate>) -> Result<Value, PluginError> {
        let mut resp = vec![];
        for (height, rate) in fees.iter() {
            if *height == 0 {
                continue;
            }
            resp.push(serde_json::json!({
                "blocks": height,
                "feerate": rate,
            }))
        }
        let floor = *fees
            .get(&0)
            .ok_or(error!("impossible get the minimum feerate"))? as i64;
        Ok(serde_json::json!({
            "feerate_floor": floor,
            "feerates": resp,
        }))
    }

    pub fn null_estimate_fees() -> Result<Value, PluginError> {
        Ok(serde_json::json!({
            "feerate_floor": 1000,
            "feerates": {},
        }))
    }
}
