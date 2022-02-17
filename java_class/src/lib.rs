#![feature(iterator_fold_self)]

use std::str::Chars;

#[cfg(test)]
mod tests {
    //TODO: tests?
}

pub mod cp_info;
pub mod cp;
pub mod attributes;
pub mod opcodes;
pub mod bytecode_tools;
pub mod methods;
pub mod fields;
pub mod class;
pub mod builders;

#[derive(Clone, PartialEq, Eq, Default)]
pub struct JVMClassName {
    name: String
}
impl JVMClassName {
    pub fn get_as_binary(&self) -> String {
        self.name.clone()
    }
    pub fn get_as_binary_str(&self) -> &str {
        &self.name
    }
    pub fn get_as_wrapped_binary(&self) -> String {
        let tmp = self.get_as_binary_str();
        if tmp.starts_with('[') || tmp.len() == 1 {
            tmp.to_owned()
        } else {
            format!("L{};", tmp)
        }
    }
    pub fn get_as_pretty(&self) -> String {
        self.name.clone().replace('/', ".")
    }
    pub fn new(name: &str) -> JVMClassName {
        if name.starts_with('L') {
            JVMClassName {name: name[1..name.len()-1].to_owned()}
        } else {
            JVMClassName {name: name.to_owned()}
        }
    }
    /// Extracts the name from a `Chars`.
    /// Only use when ref types will be wrapped in 'L' and ';'.
    pub fn extract(chars: &mut Chars) -> Option<JVMClassName> {
        Some(match chars.next()? {
            'L' => {
                let mut ans = String::with_capacity(20);
                for c in chars {
                    if c == ';' {
                        break;
                    }
                    ans.push(c);
                }
                ans.shrink_to_fit();
                JVMClassName {name: ans}
            },
            '[' => {
                let mut ans = String::with_capacity(20);
                ans.push('[');
                let mut started = false;
                for c in chars {
                    ans.push(c);
                    if c == ';' {
                        break;
                    }
                    if !started && c != '[' && c != 'L' {
                        break;
                    }
                    if c != '[' {
                        started = true;
                    }
                }
                ans.shrink_to_fit();
                JVMClassName {name: ans}
            },
            c => {
                JVMClassName {name: format!("{}", c)}
            }
        })
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct JVMFieldName {
    name: String
}
impl JVMFieldName {
    pub fn get_as_string(&self) -> String {
        self.name.clone()
    }
    pub fn get_as_str(&self) -> &str {
        &self.name
    }
    pub fn new(name: &str) -> JVMFieldName {
        JVMFieldName {name: name.to_owned()}
    }
    pub fn from_owned(name: String) -> JVMFieldName {
        JVMFieldName {name}
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct JVMFieldLocator {
    pub name: JVMFieldName,
    pub descriptor: JVMClassName
}
impl JVMFieldLocator {
    pub fn new(name: &str, descriptor: &str) -> JVMFieldLocator {
        JVMFieldLocator {
            name: JVMFieldName::new(name),
            descriptor: JVMClassName::new(descriptor)
        }
    }
    pub fn from_owned(name: String, descriptor: &str) -> JVMFieldLocator {
        JVMFieldLocator {name: JVMFieldName::from_owned(name), descriptor: JVMClassName::new(descriptor)}
    }
    pub fn get_as_binary(&self) -> String {
        format!("{} {}", self.descriptor.get_as_binary_str(), self.name.get_as_str())
    }
    pub fn get_as_pretty(&self) -> String {
        format!("{} {}", self.descriptor.get_as_pretty(), self.name.get_as_str())
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct JVMMethodLocator {
    pub name: JVMMethodName,
    pub args: Box<[JVMClassName]>,
    pub ret: JVMClassName
}
impl JVMMethodLocator {
    pub fn new(name: &str, descriptor: &str) -> JVMMethodLocator {
        let mut args = Vec::with_capacity(4);
        let args_end = descriptor.find(')').unwrap();
        let mut desc_chars = descriptor[1..args_end].chars();
        while let Some(arg) = JVMClassName::extract(&mut desc_chars) {
            args.push(arg);
        }
        args.shrink_to_fit();
        let args = args.into_boxed_slice();
        let ret = JVMClassName::extract(&mut descriptor[args_end+1..descriptor.len()].chars()).unwrap();
        JVMMethodLocator {
            name: JVMMethodName::new(name), args, ret
        }
    }
    pub fn get_as_binary(&self) -> String {
        let args: String = self.args.iter().map(|a| {
            a.get_as_wrapped_binary()
        }).collect();
        format!("{}({}){}", self.name.get_as_str(), args, self.ret.get_as_wrapped_binary())
    }
    pub fn get_as_pretty(&self) -> String {
        let args: String = self.args.iter().map(JVMClassName::get_as_wrapped_binary).reduce(|a, b| {
            format!("{}, {}", a, b)
        }).unwrap_or_else(|| "".to_owned());
        format!("{} {}({})", self.ret.get_as_pretty(), self.name.get_as_str(), args)
    }
}

#[derive(Clone, PartialEq, Eq, Default)]
pub struct JVMMethodName {
    name: String
}
impl JVMMethodName {
    pub fn get_as_string(&self) -> String {
        self.name.clone()
    }
    pub fn get_as_str(&self) -> &str {
        &self.name
    }
    pub fn new(name: &str) -> JVMMethodName {
        JVMMethodName {name: name.to_owned()}
    }
    pub fn from_owned(name: String) -> JVMMethodName {
        JVMMethodName {name}
    }
}