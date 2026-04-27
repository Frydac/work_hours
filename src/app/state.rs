use std::collections::HashMap;

// Pure week/day domain state. This file knows how the app stores visible and
// historical days, but it deliberately knows nothing about egui, auth, or
// Supabase networking.

use crate::ui;
use anyhow::{anyhow, Result};
use chrono::{Datelike, NaiveDate, Weekday};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WeekKey {
    pub(crate) year: i32,
    pub(crate) week_nr: u32,
}

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, PartialEq)]
pub struct State {
    days: Vec<ui::Day>,
    all_days: HashMap<NaiveDate, ui::Day>,
    cur_week_nr: u32,
    cur_year: i32,
}

fn current_work_week_monday() -> NaiveDate {
    let today = chrono::Local::now().date_naive();
    let week_nr = today.iso_week().week();
    let year = today.year();
    NaiveDate::from_isoywd_opt(year, week_nr, Weekday::Mon).unwrap()
}

pub(crate) fn current_iso_week_and_year() -> (u32, i32) {
    let today = chrono::Local::now().date_naive();
    let iso_week = today.iso_week();
    (iso_week.week(), iso_week.year())
}

pub(crate) fn last_iso_week_of_year(year: i32) -> u32 {
    NaiveDate::from_ymd_opt(year, 12, 28).unwrap().iso_week().week()
}

impl State {
    pub(crate) fn populate_missing_dates(&mut self) {
        let monday = current_work_week_monday();

        for (day_ix, day) in self.days.iter_mut().enumerate() {
            // Older persisted app state had no explicit date field, so rebuild
            // it from the current work-week layout when loading that data.
            if day.date.year() < 2000 {
                day.date = monday + chrono::Duration::days(day_ix as i64);
            }
        }
    }

    fn set_current_week(&mut self, week_nr: u32, year: i32) -> Result<()> {
        self.save_current_week();
        self.cur_week_nr = week_nr;
        self.cur_year = year;
        let cur_monday = NaiveDate::from_isoywd_opt(year, week_nr, Weekday::Mon).ok_or(anyhow!("invalid date"))?;
        self.days = (0..5)
            .map(|day_ix| {
                let date = cur_monday + chrono::Duration::days(day_ix);
                let mut day = ui::Day::new(date.format("%A").to_string());
                day.date = date;
                self.all_days.entry(date).or_insert(day).clone()
            })
            .collect();
        Ok(())
    }

    pub(crate) fn save_current_week(&mut self) {
        for day in &mut self.days {
            self.all_days.insert(day.date, day.clone());
        }
    }

    pub(crate) fn replace_current_week_days(&mut self, days: Vec<ui::Day>) {
        self.days = days;
        self.save_current_week();
    }

    pub(crate) fn normalize_iso_year_week(mut year: i32, mut week_nr: i32) -> (i32, u32) {
        loop {
            let max_week = last_iso_week_of_year(year) as i32;
            // ISO weeks wrap across year boundaries, so normalize the requested
            // week before rebuilding the visible week state.
            if week_nr < 1 {
                year -= 1;
                week_nr += last_iso_week_of_year(year) as i32;
                continue;
            }
            if week_nr > max_week {
                week_nr -= max_week;
                year += 1;
                continue;
            }
            return (year, week_nr as u32);
        }
    }

    pub(crate) fn set_current_week_normalized(&mut self, year: i32, week_nr: i32) {
        let (year, week_nr) = Self::normalize_iso_year_week(year, week_nr);
        let _ = self.set_current_week(week_nr, year);
    }

    pub(crate) fn shift_weeks(&mut self, nr_weeks: i32) {
        let monday = NaiveDate::from_isoywd_opt(self.cur_year, self.cur_week_nr, Weekday::Mon).unwrap();
        // Shift from the actual Monday date rather than by raw week number so
        // year transitions follow ISO week rules naturally.
        let next = monday + chrono::Duration::weeks(nr_weeks.into());
        let week = next.iso_week();
        let _ = self.set_current_week(week.week(), week.year());
    }

    pub(crate) fn jump_to_current_week(&mut self) {
        let (week_nr, year) = current_iso_week_and_year();
        let _ = self.set_current_week(week_nr, year);
    }

    pub(crate) fn current_week_key(&self) -> WeekKey {
        WeekKey {
            year: self.cur_year,
            week_nr: self.cur_week_nr,
        }
    }

    pub(crate) fn current_week_dates(&self) -> Vec<NaiveDate> {
        let monday = NaiveDate::from_isoywd_opt(self.cur_year, self.cur_week_nr, Weekday::Mon).unwrap();
        (0..5).map(|offset| monday + chrono::Duration::days(offset)).collect()
    }

    pub(crate) fn current_week_range(&self) -> (NaiveDate, NaiveDate) {
        let dates = self.current_week_dates();
        (*dates.first().unwrap(), *dates.last().unwrap())
    }

    pub(crate) fn duration(&self) -> time::Duration {
        self.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.duration())
    }

    pub(crate) fn total_target(&self) -> time::Duration {
        self.days.iter().fold(time::Duration::ZERO, |sum, day| sum + day.target())
    }

    pub(crate) fn days_mut(&mut self) -> &mut [ui::Day] {
        &mut self.days
    }

    pub(crate) fn days(&self) -> &[ui::Day] {
        &self.days
    }

    pub(crate) fn cur_year(&self) -> i32 {
        self.cur_year
    }

    pub(crate) fn cur_week_nr(&self) -> u32 {
        self.cur_week_nr
    }
}

impl Default for State {
    fn default() -> Self {
        let mut res = State {
            days: vec![],
            all_days: HashMap::new(),
            cur_week_nr: 0,
            cur_year: 0,
        };
        let (cur_week_nr, cur_year) = current_iso_week_and_year();
        let _ = res.set_current_week(cur_week_nr, cur_year);
        res
    }
}

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn default_state_has_week_target() {
        let state = State::default();
        assert_eq!(state.total_target(), time::Duration::hours(38));
    }

    #[test]
    fn normalize_iso_year_week_keeps_valid_week() {
        assert_eq!(State::normalize_iso_year_week(2026, 10), (2026, 10));
    }

    #[test]
    fn normalize_iso_year_week_wraps_forward() {
        let max_week = super::last_iso_week_of_year(2026) as i32;
        assert_eq!(State::normalize_iso_year_week(2026, max_week + 1), (2027, 1));
    }

    #[test]
    fn normalize_iso_year_week_wraps_backward() {
        let previous_year = 2025;
        let last_week = super::last_iso_week_of_year(previous_year);
        assert_eq!(State::normalize_iso_year_week(2026, 0), (previous_year, last_week));
    }

    #[test]
    fn normalize_iso_year_week_carries_forward_when_year_has_fewer_weeks() {
        assert_eq!(State::normalize_iso_year_week(2025, 53), (2026, 1));
    }
}
