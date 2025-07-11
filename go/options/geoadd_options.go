// Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0

package options

import (
	"github.com/valkey-io/valkey-glide/go/v2/constants"
	"github.com/valkey-io/valkey-glide/go/v2/internal/utils"
)

// Represents a geographic position defined by longitude and latitude
// The exact limits, as specified by `EPSG:900913 / EPSG:3785 / OSGEO:41001` are:
// - Longitude: -180 to 180 degrees
// - Latitude: -85.05112878 to 85.05112878 degrees
type GeospatialData struct {
	Latitude  float64
	Longitude float64
}

// Helper function to convert a geospatial members to geospatial data mapping to a slice of strings
// The format is: latitude, longitude, member,...
func MapGeoDataToArray(memberGeoMap map[string]GeospatialData) []string {
	result := make([]string, 0, len(memberGeoMap)*3)
	for member, geoData := range memberGeoMap {
		result = append(result, utils.FloatToString(geoData.Longitude), utils.FloatToString(geoData.Latitude), member)
	}
	return result
}

// Optional arguments to `GeoAdd` in [GeoSpatialCommands]
type GeoAddOptions struct {
	ConditionalChange constants.ConditionalSet
	Changed           bool
}

func NewGeoAddOptions() *GeoAddOptions {
	return &GeoAddOptions{}
}

// `conditionalChange` defines conditions for updating or adding elements with `ZADD` command.
func (options *GeoAddOptions) SetConditionalChange(conditionalChange constants.ConditionalSet) *GeoAddOptions {
	options.ConditionalChange = conditionalChange
	return options
}

// `Changed` changes the return value from the number of new elements added to the total number of elements changed.
func (options *GeoAddOptions) SetChanged(changed bool) *GeoAddOptions {
	options.Changed = changed
	return options
}

// `ToArgs` converts the options to a list of arguments.
func (opts *GeoAddOptions) ToArgs() ([]string, error) {
	args := []string{}
	var err error

	if opts.ConditionalChange == constants.OnlyIfExists || opts.ConditionalChange == constants.OnlyIfDoesNotExist {
		args = append(args, string(opts.ConditionalChange))
	}

	if opts.Changed {
		args = append(args, constants.ChangedKeyword)
	}

	return args, err
}
