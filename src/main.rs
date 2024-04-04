use chrono::{
    format::{parse, Parsed, StrftimeItems},
    Datelike, Local, NaiveDate,
};
use serde::{Deserialize, Serialize};
use std::{
    env,
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

// Not accounting for leap years.
const DAYS_IN_EACH_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();

    let path = env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .join("manager.json");

    let mut app = App::get_or_create(path, args);

    if app.context.args.len() <= 1 {
        app.today();
        return Ok(());
    }

    match (&app.context.args)[1].as_str() {
        "today" => {
            app.today();
            Ok(())
        }
        "reset" | "clear" => {
            Organiser::default().to_file(&app.context.path);
            println!("The manager file has been reset to the default state.");
            Ok(())
        }
        "backup" => {
            todo!()
        }
        "task" | "tasks" => app.task(),
        _ => Err(format!(
            "Did not recognise {} as a command.",
            app.context.args[1]
        )),
    }
}

// The 2 functions below were taken directly from https://www.geeksforgeeks.org/find-number-of-days-between-two-given-dates/ as I don't want to touch time stuff.

fn count_leap_years(mut year: u32, month: u32) -> u32 {
    if month <= 2 {
        year -= 1;
    }

    (year / 4) - (year / 100) + (year / 400)
}

fn calculate_days_passed(start: NaiveDate, end: NaiveDate) -> u32 {
    let start_day = start.day();
    let start_month = start.month();
    let start_year = start.year() as u32;

    let end_day = end.day();
    let end_month = end.month();
    let end_year = end.year() as u32;

    let mut n1 = start_year * 365 + start_day;
    #[allow(clippy::needless_range_loop)]
    for i in 0..start_month as usize {
        n1 += DAYS_IN_EACH_MONTH[i];
    }
    n1 += count_leap_years(start_year, start_month);

    let mut n2 = end_year * 365 + end_day;
    #[allow(clippy::needless_range_loop)]
    for i in 0..end_month as usize {
        n2 += DAYS_IN_EACH_MONTH[i];
    }
    n2 += count_leap_years(end_year, end_month);

    n2 - n1
}

fn calculate_months_passed(start: NaiveDate, end: NaiveDate) -> u32 {
    let start_month = start.month();
    let start_year = start.year() as u32;

    let end_month = end.month();
    let end_year = end.year() as u32;

    let n1 = start_year * 12 + start_month;

    let n2 = end_year * 12 + end_month;

    n2 - n1
}

fn calculate_years_passed(start: NaiveDate, end: NaiveDate) -> u32 {
    end.year() as u32 - start.year() as u32
}

struct App {
    context: Context,
    organiser: Organiser,
}

impl App {
    fn get_or_create(path: PathBuf, args: Vec<String>) -> App {
        if !Path::try_exists(&path).unwrap() {
            Organiser::default().to_file(&path)
        }

        let file = File::open(
            env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .join("manager.json"),
        )
        .unwrap();
        let reader = BufReader::new(file);

        let mut organiser: Organiser = serde_json::from_reader(reader).unwrap();

        let current_date = Local::now().date_naive();

        let mut updated_dates = false;

        if current_date != organiser.current_date {
            organiser.previous_date = organiser.current_date;
            organiser.current_date = current_date;

            updated_dates = true;
        }

        let mut app = App {
            context: Context {
                path,
                args,
                days_since_last_opened: calculate_days_passed(
                    organiser.previous_date,
                    organiser.current_date,
                ),
                months_since_last_opened: calculate_months_passed(
                    organiser.previous_date,
                    organiser.current_date,
                ),
                years_since_last_opened: calculate_years_passed(
                    organiser.previous_date,
                    organiser.current_date,
                ),
            },
            organiser,
        };

        if updated_dates {
            for task in &mut app.organiser.tasks {
                match task.how_often {
                    HowOften::Daily => {
                        let amount_to_add = if let U32WithPositiveInfinity::U32(recursions) =
                            &mut task.recursions
                        {
                            if *recursions < app.context.days_since_last_opened {
                                let temp_recursions = *recursions;
                                *recursions = 0;
                                temp_recursions
                            } else {
                                *recursions -= app.context.days_since_last_opened;
                                app.context.days_since_last_opened
                            }
                        } else {
                            app.context.days_since_last_opened
                        };

                        task.quantity_remaining += amount_to_add;
                    }
                    HowOften::Monthly => {
                        let amount_to_add = if let U32WithPositiveInfinity::U32(recursions) =
                            &mut task.recursions
                        {
                            if *recursions < app.context.months_since_last_opened {
                                let temp_recursions = *recursions;
                                *recursions = 0;
                                temp_recursions
                            } else {
                                *recursions -= app.context.months_since_last_opened;
                                app.context.months_since_last_opened
                            }
                        } else {
                            app.context.months_since_last_opened
                        };

                        task.quantity_remaining += amount_to_add;
                    }
                    HowOften::Yearly => {
                        let amount_to_add = if let U32WithPositiveInfinity::U32(recursions) =
                            &mut task.recursions
                        {
                            if *recursions < app.context.years_since_last_opened {
                                let temp_recursions = *recursions;
                                *recursions = 0;
                                temp_recursions
                            } else {
                                *recursions -= app.context.years_since_last_opened;
                                app.context.years_since_last_opened
                            }
                        } else {
                            app.context.years_since_last_opened
                        };

                        task.quantity_remaining += amount_to_add;
                    }
                    _ => todo!(),
                }
            }

            app.save();
        }

        app
    }

    fn save(&self) {
        self.organiser.to_file(&self.context.path);
    }

    fn today(&self) {
        let now = Local::now();

        println!(
            "Hello! Today is {}.",
            now.date_naive().format("%A, %B %-d, %C%y")
        );

        if self.context.days_since_last_opened > 1 {
            println!("It has been {} days, since you last opened this organiser. You will have to catch up.",self.context.days_since_last_opened);
        }

        self.display_tasks();
    }

    fn display_tasks(&self) {
        let mut daily_tasks_display = String::new();
        let mut weekly_tasks_display = String::new();
        let mut monthly_tasks_display = String::new();
        let mut yearly_tasks_display = String::new();

        for task in &self.organiser.tasks {
            if task.quantity_remaining == 0 {
                continue;
            }
            let quantity_string = if task.quantity_remaining > 1 {
                format!(" (x{})", task.quantity_remaining)
            } else {
                String::new()
            };

            //TODO: if it is the end of the week/month/year and there is a task due then, make it due today, instead of week/month/year. You know?
            match task.how_often {
                HowOften::Daily => daily_tasks_display
                    .push_str(format!("{}{},\n", task.name, quantity_string).as_str()),
                HowOften::Weekly => weekly_tasks_display
                    .push_str(format!("{}{},\n", task.name, quantity_string).as_str()),
                HowOften::Monthly => monthly_tasks_display
                    .push_str(format!("{}{},\n", task.name, quantity_string).as_str()),
                HowOften::Yearly => yearly_tasks_display
                    .push_str(format!("{}{},\n", task.name, quantity_string).as_str()),
                _ => todo!(),
            }
        }

        if !daily_tasks_display.is_empty() {
            println!("Tasks you must do sometime today:\n{}", daily_tasks_display);
        }
        if !weekly_tasks_display.is_empty() {
            println!(
                "Tasks you must do sometime this week:\n{}",
                weekly_tasks_display
            );
        }
        if !monthly_tasks_display.is_empty() {
            println!(
                "Tasks you must do sometime this month:\n{}",
                monthly_tasks_display
            );
        }
        if !yearly_tasks_display.is_empty() {
            println!(
                "Tasks you must do sometime this year:\n{}",
                yearly_tasks_display
            );
        }
    }

    fn task(&mut self) -> Result<(), String> {
        if self.context.args.len() < 3 {
            return Err(format!(
                "The command '{}' requires a sub-command.",
                self.context.args[1]
            ));
        }

        match self.context.args[2].as_str() {
            "add" => {
                if self.context.args.len() < 5 {
                    return Err(format!(
                        "The sub-command '{}' requires at least 2 arguments.",
                        self.context.args[2]
                    ));
                }

                if self.context.args.len() > 6 {
                    return Err(format!(
                        "The sub-command '{}' requires at most 3 arguments.",
                        self.context.args[2]
                    ));
                }

                if self
                    .organiser
                    .tasks
                    .iter()
                    .any(|task| task.name == self.context.args[3])
                {
                    return Err(format!(
                        "A task named '{}' already exists.",
                        self.context.args[3]
                    ));
                }

                let mut recursions = if self.context.args.len() >= 6 {
                    if let Ok(parsed) =
                        U32WithPositiveInfinity::from_str(self.context.args[5].as_str())
                    {
                        parsed
                    } else {
                        return Err(format!(
                            "Could not interpret recursions from '{}'.",
                            self.context.args[5]
                        ));
                    }
                } else {
                    U32WithPositiveInfinity::Infinity
                };

                if let U32WithPositiveInfinity::U32(value) = &mut recursions {
                    *value -= 1;
                }

                let Ok(how_often) = HowOften::from_str(&self.context.args[4]) else {
                    return Err(format!(
                        "The HowOften '{}' is not correct.",
                        self.context.args[4]
                    ));
                };

                self.organiser.tasks.push(Task {
                    name: self.context.args[3].clone(),
                    how_often,
                    quantity_remaining: 1,
                    recursions,
                });
                self.save();
                Ok(())
            }
            "complete" | "done" | "finish" => {
                if self.context.args.len() != 4 {
                    return Err(format!(
                        "The sub-command '{}' requires 1 argument.",
                        self.context.args[2]
                    ));
                }

                let Some(index) = self
                    .organiser
                    .tasks
                    .iter()
                    .position(|task| task.name == self.context.args[3])
                else {
                    return Err(format!(
                        "The task '{}' does not exist.",
                        self.context.args[3]
                    ));
                };
                self.organiser.tasks[index].quantity_remaining -= 1;

                // Specific dates don't happen more than once. No point in keeping them.
                if let HowOften::SpecificDate(_) = self.organiser.tasks[index].how_often {
                    self.organiser.tasks.swap_remove(index);
                }

                if self.organiser.tasks[index].quantity_remaining == 0 {
                    if let U32WithPositiveInfinity::U32(recursions) =
                        self.organiser.tasks[index].recursions
                    {
                        if recursions == 0 {
                            self.organiser.tasks.swap_remove(index);
                        }
                    }
                }

                self.save();
                Ok(())
            }
            "remove" | "delete" => {
                if self.context.args.len() != 4 {
                    return Err(format!(
                        "The sub-command '{}' requires 1 argument.",
                        self.context.args[2]
                    ));
                }

                let Some(index) = self
                    .organiser
                    .tasks
                    .iter()
                    .position(|task| task.name == self.context.args[3])
                else {
                    return Err(format!(
                        "The task '{}' does not exist.",
                        self.context.args[3]
                    ));
                };
                self.organiser.tasks.swap_remove(index);
                self.save();
                Ok(())
            }
            _ => Err(format!(
                "Did not recognise {} as a sub-command.",
                self.context.args[2]
            )),
        }
    }
}

struct Context {
    path: PathBuf,
    args: Vec<String>,
    days_since_last_opened: u32,
    months_since_last_opened: u32,
    years_since_last_opened: u32, // These are not in actual time, but rather crossing the line. so 2023 to 2025 would be 1 year since last opened, even if it was only 1 day apart.
}

#[derive(Serialize, Deserialize)]
struct Organiser {
    tasks: Vec<Task>,
    current_date: NaiveDate,
    previous_date: NaiveDate,
}

impl Organiser {
    fn default() -> Organiser {
        let current_date = Local::now().date_naive();
        Organiser {
            tasks: vec![],
            current_date,
            previous_date: current_date,
        }
    }

    fn to_file(&self, path: &PathBuf) {
        let mut file = File::create(path).unwrap();
        file.write_all(serde_json::to_string(self).unwrap().as_bytes())
            .unwrap();
    }
}

#[derive(Serialize, Deserialize)]
struct Task {
    name: String,
    how_often: HowOften,
    quantity_remaining: u32, // Consider making it u32. 0 for done. 1 or more for how many times we still have to do it.
    recursions: U32WithPositiveInfinity,
}

#[derive(Serialize, Deserialize)]
enum U32WithPositiveInfinity {
    U32(u32),
    Infinity,
}

impl U32WithPositiveInfinity {
    fn from_str(string: &str) -> Result<U32WithPositiveInfinity, ()> {
        let caseless_string = string.to_lowercase();
        if caseless_string == "infinity" || caseless_string == "infinite" {
            Ok(U32WithPositiveInfinity::Infinity)
        } else if let Ok(value) = caseless_string.parse() {
            Ok(U32WithPositiveInfinity::U32(value))
        } else {
            Err(())
        }
    }
}

#[derive(Serialize, Deserialize)]
enum HowOften {
    Daily,
    Weekly,
    Monthly,
    Yearly,

    SpecificWeekday(Weekday),
    SpecificDate(NaiveDate),
    SpecificDateEveryYear(DateWithoutYear),
}

impl HowOften {
    fn from_str(string: &str) -> Result<HowOften, ()> {
        let caseless_string = string.to_lowercase();
        match caseless_string.as_str() {
            "daily" => Ok(HowOften::Daily),
            "weekly" => Ok(HowOften::Weekly),
            "monthly" => Ok(HowOften::Monthly),
            "yearly" => Ok(HowOften::Monthly),

            "sunday" => Ok(HowOften::SpecificWeekday(Weekday::Sunday)),
            "monday" => Ok(HowOften::SpecificWeekday(Weekday::Monday)),
            "tuesday" => Ok(HowOften::SpecificWeekday(Weekday::Tuesday)),
            "wednesday" => Ok(HowOften::SpecificWeekday(Weekday::Wednesday)),
            "thursday" => Ok(HowOften::SpecificWeekday(Weekday::Thursday)),
            "friday" => Ok(HowOften::SpecificWeekday(Weekday::Friday)),
            "saturday" => Ok(HowOften::SpecificWeekday(Weekday::Saturday)),
            _ => {
                if let Ok(date) = NaiveDate::parse_from_str(caseless_string.as_str(), "%d/%b/%Y") {
                    Ok(HowOften::SpecificDate(date))
                } else if let Ok(date) =
                    DateWithoutYear::parse_from_str_dm(caseless_string.as_str())
                {
                    Ok(HowOften::SpecificDateEveryYear(date))
                } else {
                    Err(())
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
enum Weekday {
    Sunday,
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
}

#[derive(Serialize, Deserialize)]
struct DateWithoutYear {
    month: u8,
    day: u8,
}

impl DateWithoutYear {
    pub fn parse_from_str_dm(s: &str) -> Result<DateWithoutYear, ()> {
        const FMT: &str = "%d/%b";
        let mut parsed = Parsed::new();
        if parse(&mut parsed, s, StrftimeItems::new(FMT)).is_err() {
            return Err(());
        };
        Ok(DateWithoutYear {
            month: parsed.month().unwrap() as u8,
            day: parsed.day().unwrap() as u8,
        })
    }
}
