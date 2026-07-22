/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct NodeEffects {
    pub clip_path: Option<String>,
    pub mask: Option<String>,
    pub filter: Option<String>,
}
