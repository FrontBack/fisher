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

use fisher_common::prelude::*;


pub type Job<S> = <S as ScriptsRepositoryTrait>::Job;

pub type JobContext<S> = <
    <S as ScriptsRepositoryTrait>::Job as JobTrait<
        <S as ScriptsRepositoryTrait>::Script
    >
>::Context;

pub type JobOutput<S> = <
    <S as ScriptsRepositoryTrait>::Job as JobTrait<
        <S as ScriptsRepositoryTrait>::Script
    >
>::Output;

pub type ScriptId<S> = <
    <S as ScriptsRepositoryTrait>::Script as ScriptTrait
>::Id;
