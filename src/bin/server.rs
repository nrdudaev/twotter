#[path = "../data.rs"]
mod data;
use crate::data::*;

use rand::distr::{Alphanumeric, SampleString};
use redis::{
    AsyncCommands,
    aio::MultiplexedConnection,
};
use std::str::from_utf8;
use futures::{SinkExt, StreamExt};
use time::OffsetDateTime;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::mpsc,
};
use serde::{Serialize, Deserialize};
use tokio_util::codec::{Framed, LinesCodec, Encoder, Decoder};
use std::time::{UNIX_EPOCH, Duration};
use chrono::{DateTime, Utc};

#[tokio::main]
async fn main() {
    let mut listener: TcpListener;

    loop {
        let mut buf = String::new();
        std::io::stdin()
            .read_line(&mut buf)
            .expect("Failed to read from stdin");

        buf = buf.trim().to_lowercase();

        match buf.as_str() {
            "local" | "l" => {
                listener = TcpListener::bind("127.0.0.1:8080")
                    .await
                    .expect("failed to bind");
                break;
            }
            "global" | "g" => {
                listener = TcpListener::bind("0.0.0.0:8080")
                    .await
                    .expect("failed to bind");
                break;
            }
            _ => {
                println!("Wrong argument.\nTry 'local' or 'global'");
            }
        }
    }

    let (tx_con, mut rx_db) = mpsc::channel::<(Request, String, mpsc::Sender<(Option<String>, Result<Response, TwotterError>)>)>(8);

    let database_handler_task = tokio::spawn(async move {
        database_handler(rx_db).await;
    });

    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut framed_socket = Framed::new(socket, LinesCodec::new());
        let tx_con_clone = tx_con.clone(); // rename tx_con_clone to tx_con to check shadowing

        tokio::spawn(async move {
            connection_handler(framed_socket, tx_con_clone).await;
        });
    }

    database_handler_task.await;
}

async fn database_handler(mut rx_db: mpsc::Receiver<(Request, String, mpsc::Sender<(Option<String>, Result<Response, TwotterError>)>)>) {
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    let mut redis_connection = client.get_multiplexed_async_connection().await.unwrap();
    debug_setup(&mut redis_connection).await;

    while let Some((data, login_status, tx_db)) = rx_db.recv().await {
        if login_status.len() == 0 {
            match data {
                Request::RegistrationInfo(login, password) => {
                    let already_exists: bool = redis_connection
                        .exists(format!("user:{}", &login))
                        .await
                        .unwrap();
                    if !already_exists {
                        let _: () = redis_connection
                            .hset(format!("user:{}", &login), "password", password)
                            .await
                            .expect(format!("Failed to HSET 'user:{}'", &login).as_str());

                        tx_db.send((None, Ok(Response::None))).await;
                    } else {
                        println!("Error: UserAlreadyExists");
                        tx_db.send((None, Err(TwotterError { kind: TwotterErrorKind::UserAlreadyExists }))).await;
                    }
                }
                Request::LoginInfo(login, password) => 'label: {
                    let user_exists: bool = redis_connection
                        .exists(format!("user:{}", &login))
                        .await
                        .unwrap();
                    if !user_exists {
                        println!("Error: UserDoesntExist");
                        tx_db.send((None, Err(TwotterError { kind: TwotterErrorKind::UserDoesntExist }))).await;
                        println!("{:?}", TwotterError { kind: TwotterErrorKind::UserDoesntExist });
                        break 'label;
                    }
                    let correct_password: String = redis_connection
                        .hget(format!("user:{}", &login), "password")
                        .await
                        .unwrap();
                    if password != correct_password {
                        println!("Error: WrongPassword");
                        tx_db.send((None, Err(TwotterError { kind: TwotterErrorKind::WrongPassword }))).await;
                        break 'label;
                    }
                    tx_db.send((Some(login), Ok(Response::None))).await;
                }
                _ => {
                    println!("Error: UnloggedAccessAttempt");
                    tx_db.send((None, Err(TwotterError { kind: TwotterErrorKind::UnloggedAccessAttempt }))).await;
                }
            }
        } else {
            match data {
                Request::Post(twott) => {
                    println!("twott:{twott}");
                    let _: () = redis_connection.zadd(format!("twotts-by:{login_status}"), format!("author:{login_status}:twott:{twott}"), timestamp())
                        .await
                        .unwrap();
                    tx_db.send((None, Ok(Response::None))).await;
                }
                Request::Feed(page_num) => {
                    match redis_connection.ttl(format!("temp-feed:{login_status}")).await.unwrap() {
                        -2 => {
                            let subscription_list: Vec<String> = redis_connection.lrange(format!("subscription-list:{login_status}"), 0, -1).await.unwrap();


                            if subscription_list.len() != 0 {
                                let mut formatted_list: Vec<String> = vec![];

                                for name in subscription_list {
                                    formatted_list.push(format!("twotts-by:{name}"));
                                }

                                let _: () = redis_connection.zunionstore(format!("temp-feed:{login_status}"), formatted_list).await.unwrap();
                                let _: () = redis_connection.expire(format!("temp-feed:{login_status}"), 60).await.unwrap();
                            }
                        }
                        -1 => {
                            println!{"Error: Theoretically unreachable branch reached.\nttl = -1\n\n"};
                        }
                        _ => {
                        }
                    }
                    let number_of_twott_in_the_feed = redis_connection.zcard(format!("temp-feed:{login_status}")).await.unwrap();

                    let beginning = std::cmp::min(page_num*TWOTTS_ON_A_PAGE, number_of_twott_in_the_feed);
                    let end = std::cmp::min(page_num*TWOTTS_ON_A_PAGE + TWOTTS_ON_A_PAGE, number_of_twott_in_the_feed);
                    let unformatted_page: Vec<(String, f64)> = redis_connection.zrevrange_withscores(format!("temp-feed:{login_status}"), beginning as isize, end as isize).await.unwrap();
                    //let num_of_twotts = unformatted_page.len();
                    let num_of_pages = (number_of_twott_in_the_feed as f64 / TWOTTS_ON_A_PAGE as f64).ceil() as usize;

                    let mut output_page: Vec<(String, String, f64)> = unformatted_page.iter().map(|item| {
                        let (author, twott) = author_twott_parser(item.0.clone());
                        (author, twott, item.1)
                    }).collect();

                    tx_db.send((None, Ok(Response::Page(output_page, num_of_pages)))).await;
                }
                Request::Subscribe(name) => {
                    let _: () = redis_connection.lpush(format!("subscription-list:{login_status}"), name).await.unwrap();
                    tx_db.send((None, Ok(Response::None))).await;
                }
                Request::SubscriptionList => {
                    let list: Vec<String> = redis_connection.lrange(format!("subscription_list:{login_status}"), 0, -1).await.unwrap();
                    let mut output_list = String::new();

                    for user in list {
                        output_list.push_str(format!("{user},").as_str());
                    }

                    tx_db.send((None, Ok(Response::SubscriptionList(output_list.clone())))).await;
                }
                /*
                Request::UserList {
                    let
                }*/
                _ => { /*********************************TO HANDLE*/ }
            }
        }
    }
}

async fn connection_handler(mut framed_socket: Framed<TcpStream, LinesCodec>, tx_con: mpsc::Sender<(Request, String, mpsc::Sender<(Option<String>, Result<Response, TwotterError>)>)>) {
    let mut buf = [0; 1024];
    let mut login_status = String::new(); // len() == 0 when not logged in

    loop {
        let request: Request = framed_read(&mut framed_socket).await;
        let (tx_db, mut rx_con) = mpsc::channel::<(Option<String>, Result<Response, TwotterError>)>(8);
        tx_con.send((request, login_status.clone(), tx_db)).await;
        let (mut id_option, mut response_result) = rx_con.recv().await.expect("Failed to receive from rx_con");
        match id_option {
            Some(id) => login_status = id,
            None => {}
        }

        match response_result {
            Ok(Response::NumberOfTwotts(num_of_twotts)) => {
                framed_write(&mut framed_socket, &response_result).await;

                for i in 0..num_of_twotts {
                    (_, response_result) = rx_con.recv().await.expect("Failed to receive from rx_con within a loop");
                    framed_write(&mut framed_socket, &response_result).await;
                }
                (_, response_result) = rx_con.recv().await.expect("Failed to receive from rx_con within a loop");
                framed_write(&mut framed_socket, &response_result).await;
            }
            _ => {
                framed_write(&mut framed_socket, &response_result).await;
            }
        }
    }
}

async fn framed_read<T: Serialize + for<'a> Deserialize<'a>>(framed_socket: &mut Framed<TcpStream, LinesCodec>) -> T {
    let msg = framed_socket.next().await.expect("User disconnected.").unwrap();
    println!("read: {msg}");
    serde_json::from_str(&msg).unwrap()
}

async fn framed_write<T: Serialize + for<'a>Deserialize<'a>>(framed_socket: &mut Framed<TcpStream, LinesCodec>, msg: &T) {
    let msg_json = serde_json::to_string(&msg).unwrap();
    println!("write: {}", msg_json);
    framed_socket.send(msg_json).await.unwrap();
}

async fn debug_setup(con: &mut MultiplexedConnection) {
    let _: () = con.flushall().await.unwrap();
    let _: () = con
        .hset(format!("user:aa"), "password", "bb")
        .await
        .expect(format!("Failed to HSET 'user:aa'").as_str());

    for i in 0..5 {
        let _: () = con.zadd(format!("twotts-by:debug1"), format!("author:debug1:twott:debug twott {i}"), timestamp())
            .await
            .unwrap();
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    let _: () = con.lpush("subscription-list:aa", "debug1").await.unwrap();
}

fn author_twott_parser(mut line: String) -> (String, String) {
    line = line.split_off("author:".len());

    let author_len = line.find(":twott:").unwrap();
    let mut twott = line.split_off(author_len);
    twott = twott.split_off(":twott:".len());
    (line, twott)
}

fn timestamp() -> f64 {
    let now = OffsetDateTime::now_utc();
    let secs = now.unix_timestamp() as f64;
    let nanos = now.nanosecond() as f64;

    secs + (nanos / 1000000000.0)
}
