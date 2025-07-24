use chrono::prelude::*;
use clap::{Command, arg};
use prettytable::{Attr, Cell, Row, Table, color,format};
use std::{fs::File,io::{self,BufRead},path::Path,string::String};
use moyaposylka::Moyaposylka;

fn break_line_every_n_chars(s: &str, n: usize) -> String {//функуция для переноса строк в таблице prettytable
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        result.push(c);
        if (i + 1) % n == 0 {
            result.push('\n');
        }
    }
    result
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = Command::new("Moyaposylka")
        .version("0.1.0")
        .about("Парсер трек-номеров")
        .arg(
            arg!(-f --file <Path>)
                .required(false)
                .default_value("tracks.txt")
                .help("Путь к файлу с трек-номерами"),
        )
        .arg(
            arg!(-k --apikey <String>)
                .required(true)
                .help("API ключ для moyaposylka.ru"),
        )
        .arg(arg!(-c --csv).required(false).help("Вывод в файл csv"))
        .get_matches();

    let myfile = matches.get_one::<String>("file").expect("required");
    let api_key = matches.get_one::<String>("apikey").expect("required");
    let csv = matches.get_one::<bool>("csv").expect("required");
    let mut table = Table::new();
    table.set_format(*format::consts::FORMAT_BOX_CHARS);
    table.set_titles(Row::new(vec![
        Cell::new("Трек-номер"),
        Cell::new("Дата доставки"),
        Cell::new("Получатель"),
        Cell::new("Последний статус"),
        Cell::new("Дата статуса"),
    ]));

    let path = Path::new(myfile);
    let file = File::open(path).expect("Нет файла с трек-номерами");
    let reader = io::BufReader::new(file);
    let moyaposylka=Moyaposylka::new(api_key.to_string());
    for line in reader.lines() {
        match line {
            Ok(track) => {
                let track = track.trim();
                if track.is_empty(){
                    continue;
                }
                
                let posylka=moyaposylka.get_posylka(track).await;
                match posylka {
                    Ok(answer)=>{
                       if answer.events.len() == 0 {
                            eprintln!("Нет информации о трек-номере {}", track);
                            continue;
                        }
                        let timestamp = answer.events[0].event_date;
                        let recipient: String =
                            break_line_every_n_chars(&answer.attributes.recipient, 20);
                        let offset: FixedOffset = *Local::now().offset();

                        let naive_datetime: NaiveDateTime =//датавремя UTC
                            DateTime::from_timestamp_millis(timestamp)
                                .ok_or("Неправильный формат даты")?
                                .naive_local();
                        let time_with_offset: DateTime<Local> =//датавремя со смещением
                            DateTime::<Local>::from_naive_utc_and_offset(naive_datetime, offset);

                        table.add_row(Row::new(vec![
                            if answer.delivered {//доставленное красим в зелёный
                                Cell::new(&track).with_style(Attr::ForegroundColor(color::GREEN))
                            } else {
                                Cell::new(&track)
                            },
                            Cell::new(&answer.attributes.estimated_delivery),
                            Cell::new(&recipient),
                            Cell::new(&format!(
                                "{}\n{}",
                                answer.events[0].operation, answer.events[0].location
                            )),
                            Cell::new(&time_with_offset.fixed_offset().naive_local().to_string()),
                        ]));
                    
                    }
                    Err(er)=>{
                        println!("{}",er);
                    }
                }
            }
            Err(er) => {
                 eprintln!("{}", er);
           }
        }
    }

    if !table.is_empty() {
        table.printstd();
    };
    if *csv {
        let out = File::create("output_tracks.csv")?;
        table.to_csv(out)?;
    }

    Ok(())
}
