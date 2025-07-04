// Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0

package options

import (
	"github.com/valkey-io/valkey-glide/go/v2/constants"
	"github.com/valkey-io/valkey-glide/go/v2/internal/utils"
)

// Query for `ZRange` in [SortedSetCommands]
//   - For range queries by index (rank), use `RangeByIndex`.
//   - For range queries by lexicographical order, use `RangeByLex`.
//   - For range queries by score, use `RangeByScore`.
type ZRangeQuery interface {
	ToArgs() ([]string, error)
	dummyZRangeQuery()
}

type ZRemRangeQuery interface {
	ToArgsRemRange() []string
}

// Queries a range of elements from a sorted set by theirs index.
type RangeByIndex struct {
	Start, End int64
	Reverse    bool
}

// Queries a range of elements from a sorted set by theirs score.
type RangeByScore struct {
	Start, End scoreBoundary
	Reverse    bool
	Limit      *Limit
}

// Queries a range of elements from a sorted set by theirs lexicographical order.
type RangeByLex struct {
	Start, End lexBoundary
	Reverse    bool
	Limit      *Limit
}

type (
	scoreBoundary string
	lexBoundary   string
)

// Create a new inclusive score boundary.
func NewInclusiveScoreBoundary(bound float64) scoreBoundary {
	return scoreBoundary(utils.FloatToString(bound))
}

// Create a new score boundary.
func NewScoreBoundary(bound float64, isInclusive bool) scoreBoundary {
	if !isInclusive {
		return scoreBoundary("(" + utils.FloatToString(bound))
	}
	return scoreBoundary(utils.FloatToString(bound))
}

// Create a new score boundary defined by an infinity.
func NewInfiniteScoreBoundary(bound constants.InfBoundary) scoreBoundary {
	return scoreBoundary(string(bound) + "inf")
}

// Create a new lex boundary.
func NewLexBoundary(bound string, isInclusive bool) lexBoundary {
	if !isInclusive {
		return lexBoundary("(" + bound)
	}
	return lexBoundary("[" + bound)
}

// Create a new lex boundary defined by an infinity.
func NewInfiniteLexBoundary(bound constants.InfBoundary) lexBoundary {
	return lexBoundary(string(bound))
}

// Limit struct represents the range of elements to retrieve
// The LIMIT argument is commonly used to specify a subset of results from the matching elements, similar to the
// LIMIT clause in SQL (e.g., `SELECT LIMIT offset, count`).
type Limit struct {
	// The starting position of the range, zero based.
	Offset int64
	// The maximum number of elements to include in the range. A negative Count returns all elements from the offset.
	Count int64
}

func (limit *Limit) toArgs() ([]string, error) {
	return []string{"LIMIT", utils.IntToString(limit.Offset), utils.IntToString(limit.Count)}, nil
}

// Queries a range of elements from a sorted set by theirs index.
//
// Parameters:
//
//	start - The start index of the range.
//	end   - The end index of the range.
func NewRangeByIndexQuery(start int64, end int64) *RangeByIndex {
	return &RangeByIndex{start, end, false}
}

// Reverses the sorted set, with index `0` as the element with the highest score.
func (rbi *RangeByIndex) SetReverse() *RangeByIndex {
	rbi.Reverse = true
	return rbi
}

func (rbi *RangeByIndex) ToArgs() ([]string, error) {
	args := make([]string, 0, 3)
	args = append(args, utils.IntToString(rbi.Start), utils.IntToString(rbi.End))
	if rbi.Reverse {
		args = append(args, "REV")
	}
	return args, nil
}

func (rbi *RangeByIndex) dummyZRangeQuery() {}

// Queries a range of elements from a sorted set by theirs score.
//
// Parameters:
//
//	start - The start score of the range.
//	end   - The end score of the range.
func NewRangeByScoreQuery(start scoreBoundary, end scoreBoundary) *RangeByScore {
	return &RangeByScore{start, end, false, nil}
}

// Reverses the sorted set, with index `0` as the element with the highest score.
func (rbs *RangeByScore) SetReverse() *RangeByScore {
	rbs.Reverse = true
	return rbs
}

// The limit argument for a range query, unset by default. See [Limit] for more information.
func (rbs *RangeByScore) SetLimit(offset, count int64) *RangeByScore {
	rbs.Limit = &Limit{offset, count}
	return rbs
}

func (rbs *RangeByScore) ToArgs() ([]string, error) {
	args := make([]string, 0, 7)
	args = append(args, string(rbs.Start), string(rbs.End), "BYSCORE")
	if rbs.Reverse {
		args = append(args, "REV")
	}
	if rbs.Limit != nil {
		limitArgs, err := rbs.Limit.toArgs()
		if err != nil {
			return nil, err
		}
		args = append(args, limitArgs...)
	}
	return args, nil
}

func (rbs *RangeByScore) ToArgsRemRange() ([]string, error) {
	return []string{string(rbs.Start), string(rbs.End)}, nil
}

func (rbi *RangeByScore) dummyZRangeQuery() {}

// Queries a range of elements from a sorted set by theirs lexicographical order.
//
// Parameters:
//
//	start - The start lex of the range.
//	end   - The end lex of the range.
func NewRangeByLexQuery(start lexBoundary, end lexBoundary) *RangeByLex {
	return &RangeByLex{start, end, false, nil}
}

// Reverses the sorted set, with index `0` as the element with the highest score.
func (rbl *RangeByLex) SetReverse() *RangeByLex {
	rbl.Reverse = true
	return rbl
}

// The limit argument for a range query, unset by default. See [Limit] for more information.
func (rbl *RangeByLex) SetLimit(offset, count int64) *RangeByLex {
	rbl.Limit = &Limit{offset, count}
	return rbl
}

func (rbl *RangeByLex) ToArgs() ([]string, error) {
	args := make([]string, 0, 7)
	args = append(args, string(rbl.Start), string(rbl.End), "BYLEX")
	if rbl.Reverse {
		args = append(args, "REV")
	}
	if rbl.Limit != nil {
		limitArgs, err := rbl.Limit.toArgs()
		if err != nil {
			return nil, err
		}
		args = append(args, limitArgs...)
	}
	return args, nil
}

func (rbl *RangeByLex) ToArgsRemRange() ([]string, error) {
	return []string{string(rbl.Start), string(rbl.End)}, nil
}

func (rbl *RangeByLex) ToArgsLexCount() []string {
	return []string{string(rbl.Start), string(rbl.End)}
}

func (rbi *RangeByLex) dummyZRangeQuery() {}

// Query for `ZRangeWithScores` in [SortedSetCommands]
//   - For range queries by index (rank), use `RangeByIndex`.
//   - For range queries by score, use `RangeByScore`.
type ZRangeQueryWithScores interface {
	// A dummyZRangeQueryWithScores interface to distinguish queries for `ZRange` and `ZRangeWithScores`
	// `ZRangeWithScores` does not support BYLEX
	dummyZRangeQueryWithScores()
	dummyZRangeQuery()
	ToArgs() ([]string, error)
}

func (q *RangeByIndex) dummyZRangeQueryWithScores() {}
func (q *RangeByScore) dummyZRangeQueryWithScores() {}
