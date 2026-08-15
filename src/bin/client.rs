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
	widget::{column, Column, row, Row, container, Container, scrollable, checkbox, Text, text, text_input, button},
	Fill,
	Alignment,
	Element,
	Pixels,
	Color,
	Font,
	Length,
};
use serde::{Serialize, Deserialize};
use bytes::BytesMut;

enum State {
	Connection,
	LoginScreen,
	RegistrationScreen,
	Main,
	Users,
	MyPage,
	Page(String),
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
	current_page_num: usize,

	subscription_list: Vec<String>,
	user_list: Vec<String>,

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
							self.login_textbox.clear();
							self.password_textbox.clear();
							self.error = None;

							self.write(&Request::SubscriptionList);
							match self.read().unwrap() {
								Ok(list_response) => {
									if let Response::SubscriptionList(list) = list_response {
										self.subscription_list = list;
									}
								}
								Err(e) => {
									self.error = Some(e);
								}
							}

							self.write(&Request::Feed(0));
							match self.read().unwrap() {
								Ok(feed_response) => {
									if let Response::Page(page, num_of_pages) = feed_response {
										self.page = page;
										self.num_of_pages = num_of_pages;
									}
								}
								Err(e) => {
									self.error = Some(e);
								}
							}

							self.write(&Request::UserList);
							match self.read().unwrap() {
								Ok(user_list_response) => {
									if let Response::UserList(user_list) = user_list_response {
										self.user_list = user_list;
									}
								}
								Err(e) => {
									self.error = Some(e);
								}
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
							self.login_textbox.clear();
							self.password_textbox.clear();
							self.password_conf_textbox.clear();
						}
						Err(e) => {
							self.error = Some(e);
						}
					}
				}
			}
			Message::LogOut => {
				self.write(&Request::LogOut);
				match self.read().unwrap() {
					Ok(response) => {
						self.login_status = None;
						self.incomplete_frame = Some(BytesMut::new());
						self.page.clear();
						self.state = State::LoginScreen;
					}
					Err(e) => {
						self.error = Some(e);
					}
				}
			}
			Message::TwottChanged(twott_typed) => {
				self.twott_textbox = twott_typed;
			}
			Message::TwottSubmitted => {
				if self.twott_textbox.len() != 0 {
					self.write(&Request::Post(self.twott_textbox.clone()));
					if let Err(e) = self.read().unwrap() {
						self.error = Some(e);
					} else {
						self.twott_textbox.clear();
						self.write(&Request::Page(self.login_status.clone().unwrap(), self.current_page_num));
						match self.read().unwrap() {
							Ok(page_response) => {
								if let Response::Page(page, num_of_pages) = page_response {
									self.page = page;
									self.num_of_pages = num_of_pages;
								}
							}
							Err(e) => {
								self.error = Some(e);
							}
						}
					}
				}
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
			Message::RefreshUsers => {
				self.write(&Request::UserList);
				match self.read().unwrap() {
					Ok(user_list_response) => {
						if let Response::UserList(user_list) = user_list_response {
							self.user_list = user_list;
						}
					}
					Err(e) => {
						self.error = Some(e);
					}
				}
			}

			Message::Feed => {
				self.write(&Request::Feed(0));
				match self.read().unwrap() {
					Ok(feed_response) => {
						if let Response::Page(page, num_of_pages) = feed_response {
							self.page = page;
							self.num_of_pages = num_of_pages;
							self.state = State::Main;
						}
					}
					Err(e) => {
						self.error = Some(e);
					}
				}
			}
			Message::MyPage(current_page_num) => {
				self.write(&Request::Page(self.login_status.clone().unwrap(), 0));
				match self.read().unwrap() {
					Ok(page_response) => {
						if let Response::Page(page, num_of_pages) = page_response {
							self.page = page;
							self.num_of_pages = num_of_pages;
							self.current_page_num = current_page_num;
							self.state = State::MyPage;
						}
					}
					Err(e) => {
						self.error = Some(e);
					}
				}
			}
			Message::Page(name, current_page_num) => {
				self.write(&Request::Page(name.clone(), current_page_num));
				match self.read().unwrap() {
					Ok(page_response) => {
						if let Response::Page(page, num_of_pages) = page_response {
							self.page = page;
							self.num_of_pages = num_of_pages;
							self.current_page_num = current_page_num;
							self.state = State::Page(name.clone());
						}
					}
					Err(e) => {
						self.error = Some(e);
					}
				}
			}
			Message::Users => {
				self.write(&Request::UserList);
				match self.read().unwrap() {
					Ok(user_list_response) => {
						if let Response::UserList(user_list) = user_list_response {
							self.user_list = user_list;
							self.state = State::Users;
						}
					}
					Err(e) => {
						self.error = Some(e);
					}
				}
			}
		}
	}

	fn view(&self) -> Row<Message> {
		let err = match &self.error {
			Some(e) => {
				container(
					text(format!("Error: {}", e)).color(iced::color!(0xFF, 0x33, 0x33))
				).padding(12)
			}
			None => {
				container(text(""))
			}
		};

		let menu: iced::widget::Column<'_, Message, iced::Theme, iced::Renderer> = column![
			button("Feed")
				.on_press(Message::Feed),
			button("My Page")
				.on_press(Message::MyPage(0)),
			button("Users")
				.on_press(Message::Users),
			button("Log Out")
				.on_press(Message::LogOut),
		];

		match &self.state {
			State::Connection => {
				row![
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
						).center(Fill),
						err,
					]
				]
			}
			State::LoginScreen => {
				row![
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
							.on_press(Message::Registration),
						err
					]
				]
			}
			State::RegistrationScreen => {
				row![
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
							.on_press(Message::RegistrationRequestSubmitted),
						err
					]
				]
			}
			State::Main => {
				let mut main_screen: Vec<Element<Message>> = vec![
					text(format! ("News")).into(),
				];

				main_screen.extend(
					self.page.iter().map(|item| {
						text(format!("{}\n{}\n{}", item.0, item.1, time_converter(item.2))).into()
					})
				);

				let main_screen = Column::from_vec(main_screen);

				row![
					menu,
					main_screen,
					err
				]
			}
			State::Page(name) => {
				let mut main_screen: Vec<Element<Message>> = vec![
					text(format! ("{}", name)).into()
				];

				main_screen.extend(
					self.page.iter().map(|item| {
						text(format!("{}\n{}\n{}", item.0, item.1, time_converter(item.2))).into()
					})
				);

				let name_plus_twotts = Column::from_vec(main_screen);

				row![
					menu,
					column![
						name_plus_twotts,
						err
					]
				]
			}
			State::MyPage => {
				let mut main_screen: Vec<Element<Message>> = vec![
					text(format! ("{}", self.login_status.as_ref().unwrap())).into(),
					text_input("Type your twott here:", &self.twott_textbox)
						.on_input(Message::TwottChanged)
						.on_submit(Message::TwottSubmitted).into(),
					button("Post")
						.on_press(Message::TwottSubmitted).into(),
				];

				main_screen.extend(
					self.page.iter().map(|item| {
						text(format!("{}\n{}\n{}", item.0, item.1, time_converter(item.2))).into()
					})
				);

				let name_plus_twotts = Column::from_vec(main_screen);

				row![
					menu,
					column![
						name_plus_twotts,
						err
					]
				]
			}
			State::Users => {
				let mut users = column![];
				for user in &self.user_list {
					users = users.push(
						button(user.as_str())
							.on_press(Message::Page(user.clone(), 0))
					);
				}
				row![
					menu,
					column![
						scrollable(users)
							.height(Length::Fill),
						err
					]
				]
			}
		}
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
		self.subscription_list.push(String::from("debug1,"));
	}
}

// fn custom_label<'a>(content: impl text::IntoFragment<'a>) -> Text<'a> {
// 	text(content)
// 		.size(Pixels(18.0))
// 		.color(Color::from_rgb(0.2, 0.6, 0.3))
// 		.font(Font::DEFAULT)
// 		.width(iced::Length::Fill)
// 		.center()
// }
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
	LogOut,

	TwottChanged(String),
	TwottSubmitted,
	SearchChanged(String),
	SearchSubmitted,
	Subscribe(String),
	RefreshUsers,

	Feed,
	MyPage(usize),
	Page(String, usize),
	Users,
}

fn time_converter(time: f64) -> DateTime<Utc> {
	let secs = time as u64;
	DateTime::<Utc>::from(UNIX_EPOCH + Duration::from_secs(secs))
}

fn main() {
	iced::run(Twottr::update, Twottr::view);
}
