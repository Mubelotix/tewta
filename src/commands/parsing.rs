// Copyright (c) 2022  Mubelotix <Mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use super::*;

impl Command {
    pub fn parse(input: &str) -> Result<(Vec<usize>, Command), CommandParsingError> {
        let (input, destinators) = read_destinators(input.as_bytes())?;
        let input = unsafe { std::str::from_utf8_unchecked(input) };
        let input = input.trim();
        
        let mut args = vec!["p2pnet"];
        args.extend(input.split(' '));
        let command = Command::from_iter_safe(args)?;

        Ok((destinators, command))
    }
}

enum DestinatorItem {
    Single(usize),
    Range(usize, usize),
}

fn read_number(input: &[u8]) -> Result<(&[u8], usize), &'static str> {
    let mut i = 0;
    loop {
        if !input[i].is_ascii_digit() {
            break;
        }
        i += 1;
    }

    if i == 0 {
        return Err("No number found");
    }

    Ok((input[i..].as_ref(), unsafe {std::str::from_utf8_unchecked(&input[..i])}.parse().unwrap()))
}

fn read_destinator_item(input: &[u8]) -> Result<(&[u8], DestinatorItem), &'static str> {
    let (input, first) = read_number(input).map_err(|_| "No first number found")?;
    
    if input.starts_with(b" ") || input.starts_with(b",") {
        Ok((input, DestinatorItem::Single(first)))
    } else if input.starts_with(b"-") {
        let (input, second) = read_number(input[1..].as_ref()).map_err(|_| "No second number found")?;
        Ok((input, DestinatorItem::Range(first, second)))
    } else {
        Err("No destinator item found")
    }
}

fn read_destinators(mut input: &[u8]) -> Result<(&[u8], Vec<usize>), &'static str> {
    let mut destinator_items = Vec::new();

    loop {
        let (new_input, destinator) = read_destinator_item(input)?;
        input = new_input;
        destinator_items.push(destinator);

        if input.starts_with(b",") {
            input = &input[1..];
            continue;
        } else if input.starts_with(b" ") {
            input = &input[1..];
            break;
        } else {
            return Err("Strange character in destinator sequence");
        }
    }

    let mut destinators = Vec::new();
    for destinator_item in destinator_items {
        match destinator_item {
            DestinatorItem::Single(destinator) => destinators.push(destinator),
            DestinatorItem::Range(first, second) => {
                for destinator in first..=second {
                    destinators.push(destinator);
                }
            }
        }
    }

    Ok((input, destinators))
}
