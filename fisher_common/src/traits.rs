// Copyright (C) 2017 Pietro Albini
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

//! Traits used by Fisher.

use std::hash::Hash;
use std::sync::Arc;

use prelude::*;


/// This trait represents a script that can be run by Fisher.
pub trait ScriptTrait {
    /// The type of the ID of the script. Must be hashable.
    type Id: Hash;

    /// This method returns the unique ID of this script. The ID must be
    /// the same between calls to the same script.
    fn id(&self) -> Self::Id;

    /// This method returns if multiple instances of the script can be safely
    /// run in parallel.
    fn can_be_parallel(&self) -> bool;
}


/// This trait represents a repository of scripts.
pub trait ScriptsRepositoryTrait {
    /// The type of the scripts. Must implement
    /// [`ScriptTrait`](trait.ScriptTrait.html).
    type Script: ScriptTrait;

    /// The type of the jobs. Must implement [`JobTrait`](trait.JobTrait.html).
    type Job: JobTrait;

    /// The iterator returned by the `iter` method.
    type ScriptsIter: Iterator<Item=Arc<Self::Script>>;

    /// The iterator returned by the `jobs_after_output` method
    type JobsIter: Iterator<Item=Self::Job>;

    /// Get a script by its ID.
    fn id_exists(&self, id: &<Self::Script as ScriptTrait>::Id) -> bool;

    /// Get an iterator over all the scripts.
    fn iter(&self) -> Self::ScriptsIter;

    /// Return all the jobs generated as a conseguence of the result of another
    /// job.
    ///
    /// In Fisher, this is used to spawn status hooks when another job
    /// completes, but it can also return nothing.
    fn jobs_after_output(&self, output: <Self::Job as JobTrait>::Output)
                         -> Option<Self::JobsIter>;
}


/// This trait represents a Job that can be processed by Fisher.
pub trait JobTrait {
    /// The context that will be provided to the job.
    type Context;

    /// The output that will be returned by the job.
    type Output;

    /// The script behind this job.
    type Script: ScriptTrait;

    /// Execute the job and return the output of it.
    fn execute(&self, ctx: &Self::Context) -> Result<Self::Output>;

    /// Get the ID of the underlying script.
    fn script_id(&self) -> <Self::Script as ScriptTrait>::Id;

    /// Get the name of the underlying script.
    fn script_name(&self) -> &str;
}
