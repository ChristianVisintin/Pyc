//! ## Translator
//!
//! `translator` is the module which takes care of translating latin to russian cyrillic and viceversa

/*
*
*   Copyright (C) 2020 Christian Visintin - christian.visintin1997@gmail.com
*
* 	This file is part of "Pyc"
*
*   Pyc is free software: you can redistribute it and/or modify
*   it under the terms of the GNU General Public License as published by
*   the Free Software Foundation, either version 3 of the License, or
*   (at your option) any later version.
*
*   Pyc is distributed in the hope that it will be useful,
*   but WITHOUT ANY WARRANTY; without even the implied warranty of
*   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
*   GNU General Public License for more details.
*
*   You should have received a copy of the GNU General Public License
*   along with Pyc.  If not, see <http://www.gnu.org/licenses/>.
*
*/

use std::fmt;

#[derive(Copy, Clone, PartialEq, fmt::Debug)]
pub enum ParserError {
  MissingToken,
}

struct ParserStates {
  escape_block: bool, //Check if we're inside an escaped block (hey, keep out for expressions though)
  backslash: bool,    //Check if backslash is active
  in_expression: bool, //Check is we're inside an expression
  skip_counter: usize, //The amount of cycles to skip
  previous_state: Option<Box<ParserStates>>, //Reference to previous state
}

impl ParserStates {
  fn new(previous_state: Option<ParserStates>) -> ParserStates {
    ParserStates {
      escape_block: false,
      backslash: false,
      in_expression: false,
      skip_counter: 0,
      previous_state: match previous_state {
        None => None,
        Some(prev_state) => Some(Box::new(prev_state)),
      },
    }
  }

  fn clone(strref: &ParserStates) -> ParserStates {
    ParserStates {
      escape_block: strref.escape_block,
      backslash: strref.backslash,
      in_expression: strref.in_expression,
      skip_counter: strref.skip_counter,
      previous_state: match &strref.previous_state {
        //Recursive clone
        None => None,
        Some(state_box) => Some(Box::new(ParserStates::clone(state_box.as_ref()))),
      },
    }
  }

  fn restore_previous_state(&mut self) -> ParserStates {
    match &self.previous_state {
      None => panic!("ParserState has no previous state"),
      Some(prev_state) => ParserStates::clone(prev_state.as_ref()),
    }
  }
}

/// ### russian_to_latin
///
/// Converts a string which contains russian cyrillic characters into a latin string.
/// Characters between '"' (quotes) are escaped, expressions inside escaped blocks are translitarated anyway
/// Transliteration according to GOST 7.79-2000
pub fn russian_to_latin(input: String) -> Result<String, ParserError> {
  let mut output = String::new();
  //Iterate over string
  let mut states: ParserStates = ParserStates::new(None);
  for (i, c) in input.chars().enumerate() {
    if states.skip_counter > 0 {
      //Skip cycles
      states.skip_counter -= 1; //Decrement skip counter
      continue;
    }
    //If character is '(' an expression block starts (if backlsash is disabled)
    if c == '(' && !states.backslash {
      states.in_expression = true;
      states.escape_block = false; //Set escape to false
                                   //Create new state
      states = ParserStates::new(Some(states));
      output.push(c);
      continue;
    }
    //If character is ')' an expression ends (if backslash is disabled)
    if c == ')' && !states.backslash {
      states.in_expression = false;
      //Restore previous state
      states = match states.previous_state {
        Some(mut state) => state.restore_previous_state(),
        None => return Err(ParserError::MissingToken),
      };
      output.push(c);
      continue;
    }
    //Check if escape (and previous character is not backslash or we're inside an expression)
    if c == '"' && (!states.backslash || states.in_expression) {
      states.escape_block = !states.escape_block;
      continue;
    }
    //If backslash, enable backslash and push character
    if c == '\\' {
      states.backslash = true;
      output.push(c);
      continue;
    } else {
      states.backslash = false; //No more in backslash state
    }
    //If in escaped block, just push character
    if states.escape_block {
      output.push(c);
      continue;
    }
    //Push transliterated character
    let unchanged_str: String;
    output.push_str(match c {
      'А' => "A",
      'а' => "a",
      'Б' => "B",
      'б' => "b",
      'В' => "V",
      'в' => "v",
      'Г' => "G",
      'г' => "g",
      'Д' => "D",
      'д' => "d",
      'Е' => "E",
      'е' => "e",
      'Э' => "E",
      'э' => "e",
      'Ё' => "YO",
      'ё' => "yo",
      'Ж' => "J",
      'ж' => "j",
      'З' => "Z",
      'з' => "z",
      'И' => "I",
      'и' => "i",
      'Й' => "J",
      'й' => "j",
      'К' => {
        //K is very complex, sometimes it is C and sometimes is K
        //If following letter is in (E, I, Y), then is K
        match input.chars().nth(i + 1) {
          Some(ch) => {
            //Check following character
            match ch {
              'Е' | 'Э' | 'И' | 'Й' | 'Ы' | 'Ъ' => "K",
              ' ' => {
                //Check previous character
                match input.chars().nth(i - 1) {
                  Some(ch) => match ch {
                    'К' | 'А' | 'И' | 'О' | 'У' => "K",
                    _ => "c",
                  },
                  None => "K",
                }
              }
              _ => "C",
            }
          }
          None => {
            //Check previous character
            match input.chars().nth(i - 1) {
              Some(ch) => match ch {
                'К' | 'А' | 'И' | 'О' | 'У' => "K",
                _ => "C",
              },
              None => "K",
            }
          }
        }
      }
      'к' => {
        //K is very complex, sometimes it is C and sometimes is K
        //If following letter is in (E, I, Y), then is K
        match input.chars().nth(i + 1) {
          Some(ch) => {
            //Check following character
            match ch {
              'е' | 'э' | 'и' | 'й' | 'ы' | 'ъ' => "k",
              ' ' => {
                //Check previous character
                match input.chars().nth(i - 1) {
                  Some(ch) => match ch {
                    'к' | 'а' | 'и' | 'о' | 'у' => "k",
                    _ => "c",
                  },
                  None => "k",
                }
              }
              _ => "c",
            }
          }
          None => {
            //Check previous character
            match input.chars().nth(i - 1) {
              Some(ch) => match ch {
                'к' | 'а' | 'и' | 'о' | 'у' => "k",
                _ => "c",
              },
              None => "k",
            }
          }
        }
      }
      'Л' => "L",
      'л' => "l",
      'М' => "M",
      'м' => "m",
      'Н' => "N",
      'н' => "n",
      'О' => "O",
      'о' => "o",
      'П' => "P",
      'п' => "p",
      'Р' => "R",
      'р' => "r",
      'С' => "S",
      'с' => "s",
      'Т' => "T",
      'т' => "t",
      'У' => "U",
      'у' => "u",
      'Ф' => "F",
      'ф' => "f",
      'Х' => "H",
      'х' => "h",
      'Ч' => "CH",
      'ч' => "ch",
      'Ш' => "SH",
      'ш' => "sh",
      'Щ' => "SHH",
      'щ' => "SHH",
      'Ъ' => "",
      'ъ' => "",
      'Ы' => "Y",
      'ы' => "y",
      'Ь' => "`",
      'ь' => "`",
      'Ю' => "YU",
      'ю' => "yu",
      'Я' => "YA",
      'я' => "ya",
      '№' => "#",
      '₽' => "$",
      _ => {
        unchanged_str = c.to_string();
        unchanged_str.as_str()
      }
    });
  }
  Ok(output)
}

/// ### latin_to_russian
///
/// Converts a string which contains latin characters into a russian cyrillic string.
/// Characters between quotes are escapes
pub fn latin_to_russian(input: String) -> String {
  let mut output = String::from(input);
  //TODO: implement
  output
}

//@! Tests

#[cfg(test)]
mod tests {

  use super::*;

  #[test]
  fn test_russian_to_latin() {
    //Simple commands
    //ls -l
    let input: String = String::from("лс -л");
    let output = russian_to_latin(input.clone()).unwrap();
    println!("\"{}\" => \"{}\"", input, output);
    assert_eq!(output, "ls -l");
    //Echo hello
    let input: String = String::from("екхо хелло");
    let output = russian_to_latin(input.clone()).unwrap();
    println!("\"{}\" => \"{}\"", input, output);
    assert_eq!(output, "echo hello");
    //K vs C
    let input: String = String::from("ифконфиг етх0 аддресс 192.168.1.30 нетмаскъ 255.255.255.0"); //Use твёрдый знак to force k in netmask
    let output = russian_to_latin(input.clone()).unwrap();
    println!("\"{}\" => \"{}\"", input, output);
    assert_eq!(
      output,
      "ifconfig eth0 address 192.168.1.30 netmask 255.255.255.0"
    );
  }
}
