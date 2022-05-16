//
// Copyright (c) 2022 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

//! Subscribing primitives.
use crate::prelude::{Callback, Id, IntoHandler, KeyExpr, Sample};
use crate::sync::ZFuture;
use crate::time::Period;
use crate::API_DATA_RECEPTION_CHANNEL_SIZE;
use crate::{Result as ZResult, SessionRef};
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::RwLock;
use zenoh_protocol_core::SubInfo;
use zenoh_sync::{derive_zfuture, Runnable};

/// The subscription mode.
pub use zenoh_protocol_core::SubMode;

/// The kind of reliability.
pub use zenoh_protocol_core::Reliability;

pub(crate) struct SubscriberState {
    pub(crate) id: Id,
    pub(crate) key_expr: KeyExpr<'static>,
    pub(crate) key_expr_str: String,
    pub(crate) callback: Callback<Sample>,
}

impl fmt::Debug for SubscriberState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Subscriber{{ id:{}, key_expr:{} }}",
            self.id, self.key_expr_str
        )
    }
}

/// A subscriber that provides data through a callback.
///
/// Subscribers are automatically undeclared when dropped.
pub struct CallbackSubscriber<'a> {
    pub(crate) session: SessionRef<'a>,
    pub(crate) state: Arc<SubscriberState>,
    pub(crate) alive: bool,
}

impl CallbackSubscriber<'_> {
    /// Pull available data for a pull-mode [`CallbackSubscriber`](CallbackSubscriber).
    ///
    /// # Examples
    /// ```
    /// # async_std::task::block_on(async {
    /// use zenoh::prelude::*;
    /// use zenoh::subscriber::SubMode;
    ///
    /// let session = zenoh::open(config::peer()).await.unwrap();
    /// let subscriber = session.subscribe("/key/expression")
    ///     .callback(|sample| { println!("Received : {} {}", sample.key_expr, sample.value); })
    ///     .mode(SubMode::Pull).await.unwrap();
    /// subscriber.pull();
    /// # })
    /// ```
    #[inline]
    #[must_use = "ZFutures do nothing unless you `.wait()`, `.await` or poll them"]
    pub fn pull(&self) -> impl ZFuture<Output = ZResult<()>> {
        self.session.pull(&self.state.key_expr)
    }

    /// Close a [`CallbackSubscriber`](CallbackSubscriber).
    ///
    /// `CallbackSubscribers` are automatically closed when dropped, but you may want to use this function to handle errors or
    /// close the `CallbackSubscriber` asynchronously.
    ///
    /// # Examples
    /// ```
    /// # async_std::task::block_on(async {
    /// use zenoh::prelude::*;
    ///
    /// let session = zenoh::open(config::peer()).await.unwrap();
    /// # fn data_handler(_sample: Sample) { };
    /// let subscriber = session.subscribe("/key/expression")
    ///     .callback(data_handler).await.unwrap();
    /// subscriber.close().await.unwrap();
    /// # })
    /// ```
    #[inline]
    #[must_use = "ZFutures do nothing unless you `.wait()`, `.await` or poll them"]
    pub fn close(mut self) -> impl ZFuture<Output = ZResult<()>> {
        self.alive = false;
        self.session.unsubscribe(self.state.id)
    }
}

impl Drop for CallbackSubscriber<'_> {
    fn drop(&mut self) {
        if self.alive {
            let _ = self.session.unsubscribe(self.state.id).wait();
        }
    }
}

impl fmt::Debug for CallbackSubscriber<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.state.fmt(f)
    }
}

derive_zfuture! {
    /// A builder for initializing a [`Subscriber`](Subscriber).
    ///
    /// The result of this builder can be accessed synchronously via [`wait()`](ZFuture::wait())
    /// or asynchronously via `.await`.
    ///
    /// # Examples
    /// ```
    /// # async_std::task::block_on(async {
    /// use zenoh::prelude::*;
    ///
    /// let session = zenoh::open(config::peer()).await.unwrap();
    /// let subscriber = session
    ///     .subscribe("/key/expression")
    ///     .best_effort()
    ///     .pull_mode()
    ///     .await
    ///     .unwrap();
    /// # })
    /// ```
    #[derive(Debug, Clone)]
    pub struct SubscriberBuilder<'a, 'b> {
        pub(crate) session: SessionRef<'a>,
        pub(crate) key_expr: KeyExpr<'b>,
        pub(crate) reliability: Reliability,
        pub(crate) mode: SubMode,
        pub(crate) period: Option<Period>,
        pub(crate) local: bool,
    }
}

impl<'a, 'b> SubscriberBuilder<'a, 'b> {
    /// Make the built Subscriber a [`CallbackSubscriber`](CallbackSubscriber).
    #[inline]
    pub fn callback<Callback>(
        self,
        callback: Callback,
    ) -> CallbackSubscriberBuilder<'a, 'b, Callback>
    where
        Callback: FnMut(Sample) + Send + Sync + 'static,
    {
        CallbackSubscriberBuilder {
            session: self.session,
            key_expr: self.key_expr,
            reliability: self.reliability,
            mode: self.mode,
            period: self.period,
            local: self.local,
            callback: Some(callback),
        }
    }

    /// Make the built Subscriber a [`HandlerSubscriber`](HandlerSubscriber).
    #[inline]
    pub fn with<IntoHandler, Receiver>(
        self,
        handler: IntoHandler,
    ) -> HandlerSubscriberBuilder<'a, 'b, Receiver>
    where
        IntoHandler: crate::prelude::IntoHandler<Sample, Receiver>,
    {
        HandlerSubscriberBuilder {
            session: self.session,
            key_expr: self.key_expr,
            reliability: self.reliability,
            mode: self.mode,
            period: self.period,
            local: self.local,
            handler: Some(handler.into_handler()),
        }
    }

    /// Change the subscription reliability.
    #[inline]
    pub fn reliability(mut self, reliability: Reliability) -> Self {
        self.reliability = reliability;
        self
    }

    /// Change the subscription reliability to `Reliable`.
    #[inline]
    pub fn reliable(mut self) -> Self {
        self.reliability = Reliability::Reliable;
        self
    }

    /// Change the subscription reliability to `BestEffort`.
    #[inline]
    pub fn best_effort(mut self) -> Self {
        self.reliability = Reliability::BestEffort;
        self
    }

    /// Change the subscription mode.
    #[inline]
    pub fn mode(mut self, mode: SubMode) -> Self {
        self.mode = mode;
        self
    }

    /// Change the subscription mode to Push.
    #[inline]
    pub fn push_mode(mut self) -> Self {
        self.mode = SubMode::Push;
        self.period = None;
        self
    }

    /// Change the subscription mode to Pull.
    #[inline]
    pub fn pull_mode(mut self) -> Self {
        self.mode = SubMode::Pull;
        self
    }

    /// Change the subscription period.
    #[inline]
    pub fn period(mut self, period: Option<Period>) -> Self {
        self.period = period;
        self
    }

    /// Make the subscription local only.
    #[inline]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }
}

impl<'a> Runnable for SubscriberBuilder<'a, '_> {
    type Output = ZResult<HandlerSubscriber<'a, flume::Receiver<Sample>>>;

    fn run(&mut self) -> Self::Output {
        HandlerSubscriberBuilder {
            session: self.session.clone(),
            key_expr: self.key_expr.clone(),
            reliability: self.reliability,
            mode: self.mode,
            period: self.period,
            local: self.local,
            handler: Some(flume::bounded(*API_DATA_RECEPTION_CHANNEL_SIZE).into_handler()),
        }
        .run()
    }
}

/// A builder for initializing a [`CallbackSubscriber`](CallbackSubscriber).
///
/// The result of this builder can be accessed synchronously via [`wait()`](ZFuture::wait())
/// or asynchronously via `.await`.
///
/// # Examples
/// ```
/// # async_std::task::block_on(async {
/// use zenoh::prelude::*;
///
/// let session = zenoh::open(config::peer()).await.unwrap();
/// let subscriber = session
///     .subscribe("/key/expression")
///     .callback(|sample| { println!("Received : {} {}", sample.key_expr, sample.value); })
///     .best_effort()
///     .pull_mode()
///     .await
///     .unwrap();
/// # })
/// ```
#[derive(Clone)]
pub struct CallbackSubscriberBuilder<'a, 'b, Callback>
where
    Callback: FnMut(Sample) + Send + Sync + 'static,
{
    session: SessionRef<'a>,
    key_expr: KeyExpr<'b>,
    reliability: Reliability,
    mode: SubMode,
    period: Option<Period>,
    local: bool,
    callback: Option<Callback>,
}

impl<'a, 'b, Callback> std::future::Future for CallbackSubscriberBuilder<'a, 'b, Callback>
where
    Callback: FnMut(Sample) + Unpin + Send + Sync + 'static,
{
    type Output = <Self as Runnable>::Output;

    #[inline]
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut async_std::task::Context<'_>,
    ) -> std::task::Poll<<Self as ::std::future::Future>::Output> {
        std::task::Poll::Ready(self.run())
    }
}

impl<'a, 'b, Callback> zenoh_sync::ZFuture for CallbackSubscriberBuilder<'a, 'b, Callback>
where
    Callback: FnMut(Sample) + Unpin + Send + Sync + 'static,
{
    #[inline]
    fn wait(mut self) -> Self::Output {
        self.run()
    }
}

impl<Callback> fmt::Debug for CallbackSubscriberBuilder<'_, '_, Callback>
where
    Callback: FnMut(Sample) + Send + Sync + 'static,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackSubscriberBuilder")
            .field("session", &self.session)
            .field("key_expr", &self.key_expr)
            .field("reliability", &self.reliability)
            .field("mode", &self.mode)
            .field("period", &self.period)
            .finish()
    }
}

impl<'a, 'b, Callback> CallbackSubscriberBuilder<'a, 'b, Callback>
where
    Callback: FnMut(Sample) + Send + Sync + 'static,
{
    /// Change the subscription reliability.
    #[inline]
    pub fn reliability(mut self, reliability: Reliability) -> Self {
        self.reliability = reliability;
        self
    }

    /// Change the subscription reliability to `Reliable`.
    #[inline]
    pub fn reliable(mut self) -> Self {
        self.reliability = Reliability::Reliable;
        self
    }

    /// Change the subscription reliability to `BestEffort`.
    #[inline]
    pub fn best_effort(mut self) -> Self {
        self.reliability = Reliability::BestEffort;
        self
    }

    /// Change the subscription mode.
    #[inline]
    pub fn mode(mut self, mode: SubMode) -> Self {
        self.mode = mode;
        self
    }

    /// Change the subscription mode to Push.
    #[inline]
    pub fn push_mode(mut self) -> Self {
        self.mode = SubMode::Push;
        self.period = None;
        self
    }

    /// Change the subscription mode to Pull.
    #[inline]
    pub fn pull_mode(mut self) -> Self {
        self.mode = SubMode::Pull;
        self
    }

    /// Change the subscription period.
    #[inline]
    pub fn period(mut self, period: Option<Period>) -> Self {
        self.period = period;
        self
    }

    /// Make the subscription local onlyu.
    #[inline]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }
}

impl<'a, Callback> Runnable for CallbackSubscriberBuilder<'a, '_, Callback>
where
    Callback: FnMut(Sample) + Send + Sync + 'static,
{
    type Output = ZResult<CallbackSubscriber<'a>>;

    fn run(&mut self) -> Self::Output {
        if self.local {
            self.session
                .declare_local_subscriber(
                    &self.key_expr,
                    Arc::new(RwLock::new(self.callback.take().unwrap())),
                )
                .map(|sub_state| CallbackSubscriber {
                    session: self.session.clone(),
                    state: sub_state,
                    alive: true,
                })
        } else {
            self.session
                .declare_subscriber(
                    &self.key_expr,
                    Arc::new(RwLock::new(self.callback.take().unwrap())),
                    &SubInfo {
                        reliability: self.reliability,
                        mode: self.mode,
                        period: self.period,
                    },
                )
                .map(|sub_state| CallbackSubscriber {
                    session: self.session.clone(),
                    state: sub_state,
                    alive: true,
                })
        }
    }
}

#[derive(Clone)]
#[must_use = "ZFutures do nothing unless you `.wait()`, `.await` or poll them"]
pub struct HandlerSubscriberBuilder<'a, 'b, Receiver> {
    session: SessionRef<'a>,
    key_expr: KeyExpr<'b>,
    reliability: Reliability,
    mode: SubMode,
    period: Option<Period>,
    local: bool,
    handler: Option<crate::prelude::Handler<Sample, Receiver>>,
}

impl<'a, 'b, Receiver> std::future::Future for HandlerSubscriberBuilder<'a, 'b, Receiver>
where
    Receiver: Unpin,
{
    type Output = <Self as Runnable>::Output;

    #[inline]
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        _cx: &mut async_std::task::Context<'_>,
    ) -> std::task::Poll<<Self as ::std::future::Future>::Output> {
        std::task::Poll::Ready(self.run())
    }
}

impl<'a, 'b, Receiver> zenoh_sync::ZFuture for HandlerSubscriberBuilder<'a, 'b, Receiver>
where
    Receiver: Send + Sync + Unpin,
{
    #[inline]
    fn wait(mut self) -> Self::Output {
        self.run()
    }
}

impl<Receiver> fmt::Debug for HandlerSubscriberBuilder<'_, '_, Receiver> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HandlerSubscriberBuilder")
            .field("key_expr", &self.key_expr)
            .field("reliability", &self.reliability)
            .field("mode", &self.mode)
            .field("period", &self.period)
            .finish()
    }
}

impl<'a, 'b, Receiver> HandlerSubscriberBuilder<'a, 'b, Receiver> {
    /// Change the subscription reliability.
    #[inline]
    pub fn reliability(mut self, reliability: Reliability) -> Self {
        self.reliability = reliability;
        self
    }

    /// Change the subscription reliability to `Reliable`.
    #[inline]
    pub fn reliable(mut self) -> Self {
        self.reliability = Reliability::Reliable;
        self
    }

    /// Change the subscription reliability to `BestEffort`.
    #[inline]
    pub fn best_effort(mut self) -> Self {
        self.reliability = Reliability::BestEffort;
        self
    }

    /// Change the subscription mode.
    #[inline]
    pub fn mode(mut self, mode: SubMode) -> Self {
        self.mode = mode;
        self
    }

    /// Change the subscription mode to Push.
    #[inline]
    pub fn push_mode(mut self) -> Self {
        self.mode = SubMode::Push;
        self.period = None;
        self
    }

    /// Change the subscription mode to Pull.
    #[inline]
    pub fn pull_mode(mut self) -> Self {
        self.mode = SubMode::Pull;
        self
    }

    /// Change the subscription period.
    #[inline]
    pub fn period(mut self, period: Option<Period>) -> Self {
        self.period = period;
        self
    }

    /// Make the subscription local onlyu.
    #[inline]
    pub fn local(mut self) -> Self {
        self.local = true;
        self
    }
}

pub struct HandlerSubscriber<'a, Receiver> {
    pub subscriber: CallbackSubscriber<'a>,
    pub receiver: Receiver,
}

impl<Receiver> HandlerSubscriber<'_, Receiver> {
    #[inline]
    #[must_use = "ZFutures do nothing unless you `.wait()`, `.await` or poll them"]
    pub fn pull(&self) -> impl ZFuture<Output = ZResult<()>> {
        self.subscriber.pull()
    }
    /// Close a [`HandlerSubscriber`](HandlerSubscriber) previously created with [`subscribe`](crate::Session::subscribe).
    ///
    /// Subscribers are automatically closed when dropped, but you may want to use this function to handle errors or
    /// close the Subscriber asynchronously.
    ///
    /// # Examples
    /// ```
    /// # async_std::task::block_on(async {
    /// use zenoh::prelude::*;
    ///
    /// let session = zenoh::open(config::peer()).await.unwrap();
    /// let subscriber = session.subscribe("/key/expression").await.unwrap();
    /// subscriber.close().await.unwrap();
    /// # })
    /// ```
    #[inline]
    #[must_use = "ZFutures do nothing unless you `.wait()`, `.await` or poll them"]
    pub fn close(self) -> impl ZFuture<Output = ZResult<()>> {
        self.subscriber.close()
    }
}

impl<Receiver> Deref for HandlerSubscriber<'_, Receiver> {
    type Target = Receiver;

    fn deref(&self) -> &Self::Target {
        &self.receiver
    }
}

impl HandlerSubscriber<'_, flume::Receiver<Sample>> {
    pub fn forward<'selflifetime, E: 'selflifetime, S>(
        &'selflifetime mut self,
        sink: S,
    ) -> futures::stream::Forward<
        impl futures::TryStream<Ok = Sample, Error = E, Item = Result<Sample, E>> + 'selflifetime,
        S,
    >
    where
        S: futures::sink::Sink<Sample, Error = E>,
    {
        futures::StreamExt::forward(futures::StreamExt::map(self.receiver.stream(), Ok), sink)
    }
}

impl<'a, 'b, Receiver> Runnable for HandlerSubscriberBuilder<'a, 'b, Receiver> {
    type Output = ZResult<HandlerSubscriber<'a, Receiver>>;

    fn run(&mut self) -> Self::Output {
        let (callback, receiver) = self.handler.take().unwrap();

        let subscriber = if self.local {
            self.session
                .declare_local_subscriber(&self.key_expr, callback)
                .map(|sub_state| CallbackSubscriber {
                    session: self.session.clone(),
                    state: sub_state,
                    alive: true,
                })
        } else {
            self.session
                .declare_subscriber(
                    &self.key_expr,
                    callback,
                    &SubInfo {
                        reliability: self.reliability,
                        mode: self.mode,
                        period: self.period,
                    },
                )
                .map(|sub_state| CallbackSubscriber {
                    session: self.session.clone(),
                    state: sub_state,
                    alive: true,
                })
        };

        subscriber.map(|subscriber| HandlerSubscriber {
            subscriber,
            receiver,
        })
    }
}

impl crate::prelude::IntoHandler<Sample, flume::Receiver<Sample>>
    for (flume::Sender<Sample>, flume::Receiver<Sample>)
{
    fn into_handler(self) -> crate::prelude::Handler<Sample, flume::Receiver<Sample>> {
        let (sender, receiver) = self;
        (
            std::sync::Arc::new(std::sync::RwLock::new(move |s| {
                if let Err(e) = sender.send(s) {
                    log::warn!("Error sending sample into flume channel: {}", e)
                }
            })),
            receiver,
        )
    }
}

pub type FlumeSubscriber<'a> = HandlerSubscriber<'a, flume::Receiver<Sample>>;
