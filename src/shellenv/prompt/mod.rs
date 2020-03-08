//! # Prompt
//!
//! `prompt` is the module which takes care of processing the shell prompt

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

extern crate regex;

mod cache;
mod modules;

use cache::PromptCache;
use modules::*;
use crate::config::PromptConfig;
use super::ShellEnvironment;
use crate::translator::ioprocessor::IOProcessor;

use regex::Regex;
use std::time::Duration;

const PROMPT_KEY_REGEX: &str = r"\$\{(.*?)\}";
//Prompt keys
const PROMPT_USER: &str = "${USER}";
const PROMPT_HOSTNAME: &str = "${HOSTNAME}";
const PROMPT_WRKDIR: &str = "${WRKDIR}";
const PROMPT_LANG: &str = "${LANG}";
const PROMPT_CMDTIME: &str = "${CMD_TIME}";
const PROMPT_RC: &str = "${RC}";
//Prompt colors
pub(crate) const PROMPT_KRED: &str = "${KRED}";
pub(crate) const PROMPT_KYEL: &str = "${KYEL}";
pub(crate) const PROMPT_KGRN: &str = "${KGRN}";
pub(crate) const PROMPT_KBLU: &str = "${KBLU}";
pub(crate) const PROMPT_KCYN: &str = "${KCYN}";
pub(crate) const PROMPT_KMAG: &str = "${KMAG}";
pub(crate) const PROMPT_KBLK: &str = "${KBLK}";
pub(crate) const PROMPT_KGRY: &str = "${KGRY}";
pub(crate) const PROMPT_KWHT: &str = "${KWHT}";
pub(crate) const PROMPT_KRST: &str = "${KRST}";
//Git
const PROMPT_GIT_BRANCH: &str = "${GIT_BRANCH}";
const PROMPT_GIT_COMMIT: &str = "${GIT_COMMIT}";

/// ## ShellPrompt
/// 
/// ShellPrompt is the struct which contains the current shell prompt configuration
pub struct ShellPrompt {
    prompt_line: String,
    language: String,
    translate: bool,
    break_opt: Option<BreakOptions>,
    duration_opt: Option<DurationOptions>,
    rc_opt: Option<RcOptions>,
    git_opt: Option<GitOptions>,
    cache: PromptCache
}

/// ## ShellPrompt
/// 
/// ShellPrompt is the struct which contains the current shell prompt configuration
struct BreakOptions {
    pub break_with: String
}

/// ## DurationOptions
/// 
/// DurationOptions is the struct which contains the current duration configuration
struct DurationOptions {
    pub minimum: Duration
}

/// ## RcOptions
/// 
/// RcOptions is the struct which contains the return code configuration
struct RcOptions {
    pub ok: String,
    pub err: String
}

/// ## GitOptions
/// 
/// GitOptions is the struct which contains the current git module configuration
struct GitOptions {
    pub branch: String,
    pub commit_ref_len: usize
}

impl ShellPrompt {

    /// ### new
    /// 
    /// Instantiate a new ShellPrompt with the provided parameters
    pub fn new(language: &String, prompt_opt: &PromptConfig) -> ShellPrompt {

        let break_opt: Option<BreakOptions> = match prompt_opt.break_enabled {
            true => Some(BreakOptions::new(&prompt_opt.break_str)),
            false => None
        };
        let duration_opt: Option<DurationOptions> = match DurationOptions::should_enable(&prompt_opt.prompt_line) {
            true => Some(DurationOptions::new(prompt_opt.min_duration)),
            false => None
        };
        let rc_opt: Option<RcOptions> = match RcOptions::should_enable(&prompt_opt.prompt_line) {
            true => Some(RcOptions::new(&prompt_opt.rc_ok, &prompt_opt.rc_err)),
            false => None
        };
        let git_opt: Option<GitOptions> = match GitOptions::should_enable(&prompt_opt.prompt_line) {
            true => Some(GitOptions::new(&prompt_opt.git_branch, prompt_opt.git_commit_ref)),
            false => None
        };
        ShellPrompt {
            prompt_line: prompt_opt.prompt_line.clone(),
            language: language.clone(),
            translate: prompt_opt.translate,
            break_opt: break_opt,
            duration_opt: duration_opt,
            rc_opt: rc_opt,
            git_opt: git_opt,
            cache: PromptCache::new()
        }
    }

    /// ### print
    /// 
    /// Print prompt with resolved values
    pub fn print(&mut self, shell_env: &ShellEnvironment, processor: &IOProcessor) {
        let mut prompt_line: String = self.process_prompt(shell_env, processor);
        //Translate prompt if necessary
        if self.translate {
            prompt_line = processor.text_to_cyrillic(prompt_line);
        }
        //Write prompt
        println!("{}", prompt_line);
    }

    /// ### process_prompt
    /// 
    /// Process prompt keys and resolve prompt line
    /// Returns the processed prompt line
    /// This function is optimized to try to cache the previous values
    fn process_prompt(&mut self, shell_env: &ShellEnvironment, processor: &IOProcessor) -> String {
        let mut prompt_line: String = String::new();
        //Iterate over keys through regex ```\${(.*?)}```
        lazy_static! {
            static ref RE: Regex = Regex::new(PROMPT_KEY_REGEX).unwrap();
        }
        for regex_match in RE.captures_iter(prompt_line.clone().as_str()) {
            let mtch: String = String::from(&regex_match[0]);
            let replace_with: String = self.resolve_key(shell_env, processor, &mtch);
            prompt_line = prompt_line.replace(mtch.as_str(), replace_with.as_str());
        }
        //Invalidate cache
        self.cache.invalidate();
        //Return prompt line
        prompt_line
    }

    /// ### resolve_key
    /// 
    /// Replace the provided key with the resolved value
    fn resolve_key(&self, shell_env: &ShellEnvironment, processor: &IOProcessor, key: &String) -> String {
        match key.as_str() {
            PROMPT_CMDTIME => {
                let elapsed_time: Duration = shell_env.elapsed_time;
                match &self.duration_opt {
                    Some(opt) => {
                        if elapsed_time.as_millis() >= opt.minimum.as_millis() {
                            let millis: u128 = elapsed_time.as_millis();
                            let secs: f64 = (millis / 1000) as f64;
                            String::from(format!("took {:.1}s", secs))
                        } else {
                            String::from("")
                        }
                    },
                    None => String::from("")
                }
            },
            PROMPT_GIT_BRANCH => {
                String::from("UNSUPPORTED")
            },
            PROMPT_GIT_COMMIT => {
                String::from("UNSUPPORTED")
            },
            PROMPT_HOSTNAME => {
                shell_env.hostname.clone()
            },
            PROMPT_KBLK => {
                colors::PromptColor::Black.to_string()
            },
            PROMPT_KBLU => {
                colors::PromptColor::Blue.to_string()
            },
            PROMPT_KCYN => {
                colors::PromptColor::Cyan.to_string()
            },
            PROMPT_KGRN => {
                colors::PromptColor::Green.to_string()
            },
            PROMPT_KGRY => {
                colors::PromptColor::Gray.to_string()
            },
            PROMPT_KMAG => {
                colors::PromptColor::Magenta.to_string()
            },
            PROMPT_KRED => {
                colors::PromptColor::Red.to_string()
            },
            PROMPT_KRST => {
                colors::PromptColor::Reset.to_string()
            },
            PROMPT_KWHT => {
                colors::PromptColor::White.to_string()
            },
            PROMPT_KYEL => {
                colors::PromptColor::Yellow.to_string()
            },
            PROMPT_LANG => {
                language::language_to_str(processor.language)
            },
            PROMPT_RC => {
                match &self.rc_opt {
                    Some(opt) => {
                        match shell_env.rc {
                            0 => opt.ok.clone(),
                            _ => opt.err.clone()
                        }
                    },
                    None => String::from("")
                }
            },
            PROMPT_USER => {
                shell_env.username.clone()
            },
            PROMPT_WRKDIR => {
                shell_env.wrkdir.clone()
            },
            _ => key.clone() //Keep unresolved keys
        }
    }
}

impl BreakOptions {
    /// ### new
    /// 
    /// Instantiate a new BreakOptions with the provided parameters
    pub fn new(break_with: &String) -> BreakOptions {
        BreakOptions {
            break_with: break_with.clone()
        }
    }
}

impl DurationOptions {

    /// ### should_enable
    /// 
    /// helper which says if duration module should be enabled
    pub fn should_enable(prompt_line: &String) -> bool {
        prompt_line.contains(PROMPT_CMDTIME)
    }

    /// ### new
    /// 
    /// Instantiate a new DurationOptions with the provided parameters
    pub fn new(min_duration: usize) -> DurationOptions {
        DurationOptions {
            minimum: Duration::from_millis(min_duration as u64)
        }
    }
}

impl RcOptions {
    /// ### should_enable
    /// 
    /// helper which says if rc module should be enabled
    pub fn should_enable(prompt_line: &String) -> bool {
        prompt_line.contains(PROMPT_RC)
    }

    /// ### new
    /// 
    /// Instantiate a new RcOptions with the provided parameters
    pub fn new(ok_str: &String, err_str: &String) -> RcOptions {
        RcOptions {
            ok: ok_str.clone(),
            err: err_str.clone()
        }
    }
}

impl GitOptions {
    /// ### should_enable
    /// 
    /// helper which says if git module should be enabled
    pub fn should_enable(prompt_line: &String) -> bool {
        prompt_line.contains(PROMPT_GIT_BRANCH) || prompt_line.contains(PROMPT_GIT_COMMIT)
    }

    /// ### new
    /// 
    /// Instantiate a new GitOptions with the provided parameters
    pub fn new(branch: &String, commit: usize) -> GitOptions {
        GitOptions {
            branch: branch.clone(),
            commit_ref_len: commit
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_prompt() {

    }
}
