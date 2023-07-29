//! Generic Fee estimator for all the folgore backend.
use std::collections::HashMap;

use crate::prelude::cln::json_utils;
use crate::prelude::cln_plugin::error;
use crate::prelude::cln_plugin::errors::PluginError;
use crate::prelude::json::json;
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
    pub fn urgent_fee(fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&6).copied()
    }

    pub fn hightest_fee(fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&2).copied()
    }

    pub fn normal_fee(fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&12).copied()
    }

    pub fn slow_fee(fees: &HashMap<u64, FeeRate>) -> Option<FeeRate> {
        fees.get(&100).copied()
    }

    pub fn build_estimate_fees(fees: &HashMap<u64, FeeRate>) -> Result<Value, PluginError> {
        let mut resp = json_utils::init_payload();
        // FIXME: move all the error here in plugin logs and
        // return the null fees estimation
        let Some(high) = Self::hightest_fee(fees) else {
            return Err(error!("highest fee not found"));
        };
        let Some(urgent) = Self::urgent_fee(fees) else {
            return Err(error!("urgent fee not found"));
        };
        let Some(normal) = Self::normal_fee(fees) else {
            return Err(error!("normal fee not found"));
        };
        let Some(slow) = Self::slow_fee(fees) else {
            return Err(error!("slow fee not found"));
        };
        json_utils::add_number(&mut resp, "opening", normal as i64);
        json_utils::add_number(&mut resp, "mutual_close", slow as i64);
        json_utils::add_number(&mut resp, "unilateral_close", urgent as i64);
        json_utils::add_number(&mut resp, "delayed_to_us", normal as i64);
        json_utils::add_number(&mut resp, "htlc_resolution", urgent as i64);
        json_utils::add_number(&mut resp, "penalty", normal as i64);
        json_utils::add_number(&mut resp, "min_acceptable", (slow / 2) as i64);
        json_utils::add_number(&mut resp, "max_acceptable", (high * 2) as i64);
        Ok(resp)
    }

    pub fn null_estimate_fees() -> Result<Value, PluginError> {
        Ok(json!({
            "opening": null,
            "mutual_close": null,
            "unilateral_close": null,
            "delayed_to_us": null,
            "htlc_resolution": null,
            "penalty": null,
            "min_acceptable": null,
            "max_acceptable": null,
        }))
    }
}
