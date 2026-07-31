#[path = "../data.rs"]
mod data;
use crate::data::*;

use std::{
	net::TcpStream,
	io::{Write, Read},
	str::from_utf8,
	time::{UNIX_EPOCH, Duration},
};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use tokio_util::codec::{LinesCodec, Encoder, Decoder};
use iced::{
	widget::{column, container, checkbox, Container, Column, text, text_input, button},
	Fill,
	Alignment,
	Element,
};
use serde::{Serialize, Deserialize};
use bytes::BytesMut;

enum State {
	Connection,
	LoginScreen,
	RegistrationScreen,
	Main,
}

impl Default for State {
	fn default() -> Self {
		State::Connection
	}
}

#[derive(Default)]
struct Twottr {
	state: State,

	address_textbox: String,
	connection: Option<TcpStream>,
	codec: LinesCodec,
	incomplete_frame: Option<BytesMut>,
	connection_error: bool,

	login_status: Option<String>,
	login_textbox: String,
	password_textbox: String,
	password_conf_textbox: String,
	show_password: bool,

	twott_textbox: String,
	search_textbox: String,
	page: Vec<(String, String, f64)>,
	num_of_pages: usize,
	current_page: usize,

	subscription_list: String,

	error: Option<TwotterError>,
}

impl Twottr {
	fn update(&mut self, message: Message) {
		match message {
			Message::AddressChanged(address_typed) => {
				self.address_textbox = address_typed;
			}
			Message::AddressSubmitted => {
				match TcpStream::connect(self.address_textbox.clone()) {
					Ok(con) => {
						self.connection = Some(con);
						self.incomplete_frame = Some(BytesMut::new());
						self.connection_error = false;
						self.address_textbox.clear();
						self.state = State::LoginScreen;
					}
					Err(_) => {
						self.connection_error = true;
					}
				}
			}
			Message::LocalServer => {
				match TcpStream::connect("127.0.0.1:8080") {
					Ok(con) => {
						self.connection = Some(con);
						self.incomplete_frame = Some(BytesMut::new());
						self.connection_error = false;
						self.address_textbox.clear();
						self.state = State::LoginScreen;
					}
					Err(_) => {
						self.connection_error = true;
					}
				}
			}
			Message::LoginChanged(login_typed) => {
				self.login_textbox = login_typed;
			}
			Message::PasswordChanged(password_typed) => {
				self.password_textbox = password_typed;
			}
			Message::PasswordConfirmationChanged(password_conf_typed) => {
				self.password_conf_textbox = password_conf_typed;
			}
			Message::LoginRequestSubmitted => {
				self.debugger();
				if self.login_textbox.len() != 0 && self.password_textbox.len() != 0 {
					let login_attempt = Request::LoginInfo(self.login_textbox.clone(), self.password_textbox.clone());
					println!("login_attempt: {:?}", login_attempt);
					self.write(&login_attempt);

					let login_result: Result<Response, TwotterError> = self.read().unwrap();

					match login_result {
						Ok(Response) => {
							self.state = State::Main;
							self.login_status = Some(self.login_textbox.clone());
							self.error = None;
							self.write(&Request::SubscriptionList);
							let subscription_list_result: Result<Response, TwotterError> = self.read().unwrap();
							match subscription_list_result {
								Ok(list_response) => {
									if let Response::SubscriptionList(list) = list_response {
										self.subscription_list = list;
									}
								}
								Err(e) => {
									self.error = Some(e);
								}
							}
							// self.get_feed(0);
							self.write(&Request::Feed(0));
							if let Ok(Response::Page(page_vec, num_of_pages)) = self.read().unwrap() {
								self.page = page_vec;
								self.num_of_pages = num_of_pages;
							}
						}
						Err(e) => {
							self.error = Some(e);
						}
					}
				}
			}
			Message::Registration => {
				self.state = State::RegistrationScreen;
			}
			Message::RegistrationRequestSubmitted => {
				if self.login_textbox.len() != 0
					&& self.password_textbox.len() != 0
					&& self.password_conf_textbox.len() != 0
					&& self.password_textbox == self.password_conf_textbox
				{
					let registration_info = Request::RegistrationInfo(self.login_textbox.clone(), self.password_textbox.clone());
					self.write(&registration_info);

					let registration_result: Result<Response, TwotterError> = self.read().unwrap();
					match registration_result {
						Ok(reg_res) => {
							self.state = State::LoginScreen;
						}
						Err(e) => {
							self.error = Some(e);
						}
					}
				}
			}
			Message::TwottChanged(twott_typed) => {
				self.twott_textbox = twott_typed;
			}
			Message::TwottSubmitted => {
				self.write(&Request::Post(self.twott_textbox.clone()));
			}
			Message::SearchChanged(search_typed) => {

			}
			Message::SearchSubmitted => {

			}
			Message::ShowPassword(_) => {
				self.show_password = !self.show_password;
			}
			Message::Subscribe(name) => {
				self.write(&Request::Subscribe(name));
			}
		}
	}

	fn view(&self) -> Column<Message> {
		let mut res = match self.state {
			State::Connection => {
				column![
					container(
						column![
							text_input("Server address", &self.address_textbox)
								.on_input(Message::AddressChanged)
								.on_submit(Message::AddressSubmitted)
								.align_x(Alignment::Center)
								.width(200)
								.size(25),
							container(
								  button("Local").on_press(Message::LocalServer)
							),
						]
					).center(Fill)
				]
			},
			State::LoginScreen => {
				column![
					text_input("Login", &self.login_textbox)
						.on_input(Message::LoginChanged)
						.on_submit(Message::LoginRequestSubmitted),
					text_input("Password", &self.password_textbox)
						.on_input(Message::PasswordChanged)
						.secure(!self.show_password)
						.on_submit(Message::LoginRequestSubmitted),
					checkbox(self.show_password)
						.label("Show password")
						.on_toggle(Message::ShowPassword),
					button("Login")
						.on_press(Message::LoginRequestSubmitted),
					text(format!("If you don't have an account,\nregister first.\n")),
					button("Registration")
						.on_press(Message::Registration)
				]
			},
			State::RegistrationScreen => {
				column![
					text_input("Login", &self.login_textbox)
						.on_input(Message::LoginChanged)
						.on_submit(Message::RegistrationRequestSubmitted),
					text_input("Password", &self.password_textbox)
						.on_input(Message::PasswordChanged)
						.secure(!self.show_password)
						.on_submit(Message::RegistrationRequestSubmitted),
					text_input("Repeat the password", &self.password_conf_textbox)
						.on_input(Message::PasswordConfirmationChanged)
						.secure(!self.show_password)
						.on_submit(Message::RegistrationRequestSubmitted),
					checkbox(self.show_password)
						.label("Show password")
						.on_toggle(Message::ShowPassword),
					button("Register")
						.on_press(Message::RegistrationRequestSubmitted)
				]
			},
			State::Main => {
				let mut elements = vec![
					text(format!("{}", self.login_status.clone().unwrap())).into()
				];

				elements.extend(
					self.page.iter().map(|item| {
						text(format!("{}\n{}\n{}", item.0, item.1, time_converter(item.2))).into()
					})
				);

				Column::from_vec(elements).into()
			},
		};

		if let Some(err) = &self.error {
			res = res.push(
				container(
					text(format!("Error: {}", err)).color(iced::color!(0xFF, 0x33, 0x33))
				).padding(12)
			);
		}

		res.into()
	}

	fn read(&mut self) -> Result<Result<Response, TwotterError>, serde_json::Error> {
		let mut read_buffer = [0u8; 1024];

		loop{
			if let Some(frame) = self.codec.decode(&mut self.incomplete_frame.as_mut().unwrap()).unwrap() {
				return serde_json::from_str::<Result<Response, TwotterError>>(&frame);
			}

			let n = self.connection.as_ref().unwrap().read(&mut read_buffer).unwrap();

			if n == 0 {
				panic!("n == 0");
			}

			self.incomplete_frame.as_mut().unwrap().extend_from_slice(&read_buffer[..n]);
		}
	}

	fn write(&mut self, message: &Request) {
		let msg_json = serde_json::to_string(message).unwrap();
		let mut buffer = BytesMut::new();

		self.codec.encode(msg_json, &mut buffer).unwrap();

		self.connection.as_ref().unwrap().write_all(&buffer).unwrap();
		self.connection.as_ref().unwrap().flush().unwrap();
	}

	fn debugger(&mut self) {
		self.subscription_list.push_str("debug1,");
	}
}
/*
fn twott_layout<'a>(container: Container<'a>) -> Container<'a> {
	container.padding(Padding::from(15))
	//.width(Length::Fixed())
	.align_x(Horizontal::Center)
	.align_y(Vertical::Center)
	.style()
}*/

// const TWOTT_STYLE: container::Style {
// 	background: Some(Background::Gradient(
// 		iced::gradient::Linear::new(std::f32::consts::FRAC_PI_4)
// 			.add_stop(0.0, Color::from_rgb8())
// 			.add_stop(0.0, Color::from_rgb8())
// 			.into(),
// 	)),
// 	border: iced::Border {
//
// 	}
// 	shadow:
// 	text_color
// 	snap: true,
// }

#[derive(Debug, Clone)]
enum Message {
	LocalServer,
	AddressChanged(String),
	AddressSubmitted,

	LoginChanged(String),
	PasswordChanged(String),
	PasswordConfirmationChanged(String),
	LoginRequestSubmitted,
	Registration,
	RegistrationRequestSubmitted,
	ShowPassword(bool),

	TwottChanged(String),
	TwottSubmitted,
	SearchChanged(String),
	SearchSubmitted,
	Subscribe(String),
}

fn time_converter(time: f64) -> DateTime<Utc> {
	let secs = time as u64;
	DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(secs))
}

fn main() {
	iced::run(Twottr::update, Twottr::view);
}
