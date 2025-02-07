// Copyright (C) 2016-2017 Pietro Albini
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

use std::net::IpAddr;
use std::str::FromStr;
use std::fs::File;
use std::io::Write;

use providers::prelude::*;
use fisher_common::prelude::*;


#[derive(Debug)]
pub struct TestingProvider {
    config: String,
}

impl ProviderTrait for TestingProvider {

    fn new(config: &str) -> Result<Self> {
        // If the configuration is "yes", then it's correct
        if config != "FAIL" {
            Ok(TestingProvider {
                config: config.into(),
            })
        } else {
            // This error doesn't make any sense, but it's still an error
            Err(ErrorKind::ProviderNotFound(String::new()).into())
        }
    }

    fn validate(&self, request: &Request) -> RequestType {
        let req;
        if let &Request::Web(ref inner) = request {
            req = inner;
        } else {
            return RequestType::Invalid;
        }

        // If the secret param is provided, validate it
        if let Some(secret) = req.params.get("secret") {
            if secret != "testing" {
                return RequestType::Invalid;
            }
        }

        // If the ip param is provided, validate it
        if let Some(ip) = req.params.get("ip") {
            if req.source != IpAddr::from_str(ip).unwrap() {
                return RequestType::Invalid;
            }
        }

        // Allow to override the result of this
        if let Some(request_type) = req.params.get("request_type") {
            match request_type.as_ref() {
                // "ping" will return RequestType::Ping
                "ping" => {
                    return RequestType::Ping;
                },
                _ => {}
            }
        }

        RequestType::ExecuteHook
    }

    fn env(&self, request: &Request) -> HashMap<String, String> {
        let mut res = HashMap::new();

        let req;
        if let &Request::Web(ref inner) = request {
            req = inner;
        } else {
            return res;
        }

        // Return the provided env
        if let Some(env) = req.params.get("env") {
            res.insert("ENV".to_string(), env.clone());
        }

        res
    }

    fn prepare_directory(&self, _req: &Request, path: &PathBuf)
                         -> Result<()> {
        // Create a test file
        let mut dest = path.clone();
        dest.push("prepared");
        writeln!(File::create(&dest)?, "prepared")?;

        Ok(())
    }

    fn trigger_status_hooks(&self, request: &Request) -> bool {
        if let &Request::Web(ref inner) = request {
            ! inner.params.contains_key("ignore_status_hooks")
        } else {
            true
        }
    }
}


#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::IpAddr;
    use std::str::FromStr;

    use utils::testing::*;
    use requests::RequestType;
    use providers::ProviderTrait;

    use super::TestingProvider;


    #[test]
    fn test_new() {
        assert!(TestingProvider::new("").is_ok());
        assert!(TestingProvider::new("SOMETHING").is_ok());
        assert!(TestingProvider::new("FAIL").is_err());
    }


    #[test]
    fn test_validate() {
        let p = TestingProvider::new("").unwrap();

        // Without any secret
        assert_eq!(p.validate(&dummy_web_request().into()), RequestType::ExecuteHook);

        // With the wrong secret
        let mut req = dummy_web_request();
        req.params.insert("secret".to_string(), "wrong!!!".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::Invalid);

        // With the correct secret
        let mut req = dummy_web_request();
        req.params.insert("secret".to_string(), "testing".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::ExecuteHook);

        // With the wrong IP address
        let mut req = dummy_web_request();
        req.params.insert("ip".into(), "127.1.1.1".into());
        req.source = IpAddr::from_str("127.2.2.2").unwrap();
        assert_eq!(p.validate(&req.into()), RequestType::Invalid);

        // With the right IP address
        let mut req = dummy_web_request();
        req.params.insert("ip".into(), "127.1.1.1".into());
        req.source = IpAddr::from_str("127.1.1.1").unwrap();
        assert_eq!(p.validate(&req.into()), RequestType::ExecuteHook);

        // With the request_type param but with no meaningful value
        let mut req = dummy_web_request();
        req.params.insert("request_type".to_string(), "something".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::ExecuteHook);

        // With the request_type param and the "ping" value
        let mut req = dummy_web_request();
        req.params.insert("request_type".to_string(), "ping".to_string());
        assert_eq!(p.validate(&req.into()), RequestType::Ping);
    }


    #[test]
    fn test_env() {
        let p = TestingProvider::new("").unwrap();

        // Without the env param
        assert_eq!(p.env(&dummy_web_request().into()), HashMap::new());

        // With the env param
        let mut req = dummy_web_request();
        req.params.insert("env".to_string(), "test".to_string());

        let mut should_be = HashMap::new();
        should_be.insert("ENV".to_string(), "test".to_string());

        assert_eq!(p.env(&req.into()), should_be);
    }

    #[test]
    fn test_trigger_status_hooks() {
        let p = TestingProvider::new("").unwrap();

        assert!(p.trigger_status_hooks(&dummy_web_request().into()));

        let mut req = dummy_web_request();
        req.params.insert("ignore_status_hooks".into(), "yes".into());

        assert!(! p.trigger_status_hooks(&req.into()));
    }
}
