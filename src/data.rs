use serde::{Serialize, Deserialize};
use std::fmt;
use std::any::Any;

pub const TWOTTS_ON_A_PAGE: usize = 10;

#[derive(Serialize, Deserialize, Debug)]
pub enum Request {
	LoginInfo(String, String),
	RegistrationInfo(String, String),
	Post(String),
	Feed(usize),	// a usize is a page number
	Subscribe(String),
	UserList,
	SubscriptionList,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Response {
	NumberOfTwotts(usize),
	NumberOfPages(usize),
	Twott(String, String, f64), // (author, twott, timestamp)
	Page(Vec<(String, String, f64)>, usize), // the usize is the number of pages
	SubscriptionList(String),
	None,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TwotterError {
	pub kind: TwotterErrorKind,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum TwotterErrorKind {
	UserAlreadyExists,
	UserDoesntExist,
	WrongPassword,
	UnloggedAccessAttempt,

	ErrorParsingResponse,
}

impl fmt::Display for TwotterError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self.kind {
			TwotterErrorKind::UserAlreadyExists => write!(f, "User already exists"),
			TwotterErrorKind::UserDoesntExist => write!(f, "User doesn't exist"),
			TwotterErrorKind::WrongPassword => write!(f, "Wrong password"),
			TwotterErrorKind::UnloggedAccessAttempt => write!(f, "Unlogged access attempt"),
			TwotterErrorKind::ErrorParsingResponse => write!(f, "Error parsing response")
		}
	}
}
