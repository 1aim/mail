use std::io::{self, BufRead, BufReader, StdinLock, StdoutLock, Write};
use std::str::FromStr;

use rpassword::read_password_with_reader;

use mail::{
    header_components::{Phrase, Unstructured},
    smtp::misc::Domain,
    Email, HeaderTryFrom, Mailbox,
};

pub struct ClDialog<'a> {
    stdin: BufReader<StdinLock<'a>>,
    stdout: StdoutLock<'a>,
}

pub fn with_dialog<FN, R>(func: FN) -> R
where
    FN: for<'a> FnOnce(ClDialog<'a>) -> R,
{
    let stdin = io::stdin();
    let stdout = io::stdout();
    let dialog = ClDialog::new(stdin.lock(), stdout.lock());
    func(dialog)
}

impl<'a> ClDialog<'a> {
    pub fn new(stdin: StdinLock<'a>, stdout: StdoutLock<'a>) -> Self {
        ClDialog {
            stdin: BufReader::new(stdin),
            stdout,
        }
    }

    pub fn stdout(&mut self) -> &mut StdoutLock<'a> {
        &mut self.stdout
    }

    // pub fn stdin(&mut self) -> &mut BufReader<StdinLock<'a>> {
    //     &mut self.stdin
    // }

    pub fn prompt(&mut self, prompt: &str) -> Result<(), io::Error> {
        write!(self.stdout, "{}: ", prompt)?;
        self.stdout.flush()
    }

    pub fn read_line(&mut self) -> Result<String, io::Error> {
        let mut line = String::new();
        self.stdin.read_line(&mut line)?;
        Ok(line)
    }

    pub fn read_email(&mut self) -> Result<Email, io::Error> {
        loop {
            let line = self.read_line()?;
            let line = line.trim();
            if let Ok(email) = Email::try_from(line) {
                return Ok(email);
            }
            self.prompt("[syntax error] email")?;
        }
    }

    pub fn read_opt_phrase(&mut self) -> Result<Option<Phrase>, io::Error> {
        loop {
            let line = self.read_line()?;
            let line = line.trim();
            if line.is_empty() {
                return Ok(None);
            }
            if let Ok(phrase) = Phrase::try_from(line) {
                return Ok(Some(phrase));
            }
            self.prompt("[syntax error] optional phrase")?;
        }
    }

    pub fn read_mailbox(&mut self) -> Result<Mailbox, io::Error> {
        self.prompt("- Email Address")?;
        let email = self.read_email()?;
        self.prompt("- Display Name")?;
        let display_name = self.read_opt_phrase()?;
        Ok(Mailbox::from((display_name, email)))
    }

    pub fn read_password(&mut self) -> Result<String, io::Error> {
        read_password_with_reader(Some(&mut self.stdin))
    }

    pub fn read_mail_text_body(&mut self) -> Result<String, io::Error> {
        let mut buf = String::new();
        while self.stdin.read_line(&mut buf)? != 0 {
            if !buf.ends_with("\r\n") {
                if buf.ends_with("\r") {
                    buf.push('\n')
                } else if buf.ends_with("\n") {
                    let n_idx = buf.len() - 1;
                    buf.insert(n_idx, '\r');
                } else {
                    buf.push_str("\r\n");
                }
            }
        }
        Ok(buf)
    }

    // pub fn read_ip_addr(&mut self) -> Result<IpAddr, io::Error> {
    //     loop {
    //         let line = self.read_line()?;
    //         let line = line.trim();
    //         if let Ok(ip_addr) = IpAddr::from_str(line) {
    //             return Ok(ip_addr)
    //         }
    //         self.prompt("[syntax error] address")?;
    //     }
    // }

    pub fn read_domain(&mut self) -> Result<Domain, io::Error> {
        loop {
            let line = self.read_line()?;
            let line = line.trim();
            if let Ok(domain) = Domain::from_str(line) {
                return Ok(domain);
            }
            self.prompt("[syntax error] domain")?;
        }
    }

    pub fn read_auth_data(&mut self) -> Result<AuthData, io::Error> {
        self.prompt("- username")?;
        let username = self.read_line()?.trim().to_owned();
        self.prompt("- password")?;
        let password = self.read_password()?;
        Ok(AuthData { username, password })
    }

    pub fn read_msa_info(&mut self) -> Result<MsaInfo, io::Error> {
        writeln!(self.stdout, "Mail Submission Agent (MSA) Information:")?;
        self.prompt("MSA domain name")?;
        let domain = self.read_domain()?;
        writeln!(self.stdout, "[using port 587 and STARTTLS]")?;
        writeln!(self.stdout, "MSA Authentication data")?;
        let auth = self.read_auth_data()?;
        Ok(MsaInfo { domain, auth })
    }

    pub fn read_simple_mail(&mut self) -> Result<SimpleMail, io::Error> {
        writeln!(self.stdout, "From/Sender:")?;
        let from = self.read_mailbox()?;
        writeln!(self.stdout, "To/Recipient")?;
        let to = self.read_mailbox()?;
        self.prompt("Subject")?;
        let subject = Unstructured::try_from(self.read_line()?.trim()).unwrap();

        writeln!(self.stdout, "Utf-8 text body [end with Ctrl-D]:")?;
        self.stdout.flush()?;
        let text_body = self.read_mail_text_body()?;
        Ok(SimpleMail {
            from,
            to,
            subject,
            text_body,
        })
    }

    pub fn read_yn(&mut self) -> Result<bool, io::Error> {
        loop {
            let line = self.read_line()?;
            let line = line.trim();
            let valid = match line {
                "y" => true,
                "n" => false,
                _ => continue,
            };
            return Ok(valid);
        }
    }
}

/// POD
#[derive(Debug)]
pub struct AuthData {
    pub username: String,
    pub password: String,
}

/// POD
#[derive(Debug)]
pub struct MsaInfo {
    pub domain: Domain,
    pub auth: AuthData,
}

/// POD
#[derive(Debug)]
pub struct SimpleMail {
    pub from: Mailbox,
    pub to: Mailbox,
    pub subject: Unstructured,
    pub text_body: String,
}
