use std::process::Command;

use futures::TryStreamExt;
use inotify::EventMask;
use rsincronlib::handler::Handler;

fn expand_variables(
    input: &str,
    filename: &str,
    path: &str,
    mask_text: &str,
    mask_bits: &str,
) -> String {
    let mut formatted = String::new();
    let mut dollar = false;
    for c in input.chars() {
        if c == '$' {
            if !dollar {
                dollar = true;
            } else {
                formatted.push(c);
                dollar = false;
            }
        } else {
            if dollar {
                match c {
                    '#' => formatted.push_str(filename),
                    '@' => formatted.push_str(path),
                    '%' => formatted.push_str(mask_text),
                    '&' => formatted.push_str(mask_bits),
                    _ => formatted.push(c),
                }
                dollar = false;
            } else {
                formatted.push(c);
            }
        }
    }
    formatted
}

/* TODO: check out Arc, Mutex and all that jazz
async fn process_event_stream(stream: EventStream<[u8; 1024]>) {
    while let Ok(event) = stream.try_next().await {
        let Some(event) = event else {
            continue;
        };

        let (path, mask, command) = handler.active_watches.get(&event.wd).unwrap().clone();
        let filename = match event.name {
            Some(string) => string.to_str().unwrap_or_default().to_owned(),
            _ => String::default(),
        };

        if event.mask == EventMask::CREATE | EventMask::ISDIR {
            handler.add_watch(
                (path.to_owned(), mask.to_owned(), command.clone()),
                Some(filename.to_string()),
            );
        }

        let masks = format!("{:?}", event.mask).replace(" | ", ",");
        let command = expand_variables(
            &command,
            &filename,
            &path.to_string(),
            &masks,
            &event.mask.bits().to_string(),
        );

        let _ = Command::new("bash").arg("-c").arg(command).spawn();
    }
}
*/

#[async_std::main]
async fn main() {
    let mut handler = Handler::setup(Handler::new().unwrap()).unwrap();

    while let Ok(event) = handler.get_event_stream().try_next().await {
        let Some(event) = event else {
            continue;
        };

        let (path, mask, command) = handler.active_watches.get(&event.wd).unwrap().clone();
        let filename = match event.name {
            Some(string) => string.to_str().unwrap_or_default().to_owned(),
            _ => String::default(),
        };

        if event.mask == EventMask::CREATE | EventMask::ISDIR {
            handler.add_watch(
                (path.to_owned(), mask.to_owned(), command.clone()),
                Some(filename.to_string()),
            );
        }

        let masks = format!("{:?}", event.mask).replace(" | ", ",");
        let command = expand_variables(
            &command,
            &filename,
            &path.to_string(),
            &masks,
            &event.mask.bits().to_string(),
        );

        let _ = Command::new("bash").arg("-c").arg(command).spawn();
    }
}
