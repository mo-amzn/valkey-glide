// Copyright Valkey GLIDE Project Contributors - SPDX Identifier: Apache-2.0

package api

import (
	"fmt"
	"testing"
	"time"

	"github.com/stretchr/testify/assert"
	"github.com/valkey-io/valkey-glide/go/protobuf"
)

func TestDefaultStandaloneConfig(t *testing.T) {
	config := NewGlideClientConfiguration()
	expected := &protobuf.ConnectionRequest{
		TlsMode:            protobuf.TlsMode_NoTls,
		ClusterModeEnabled: false,
		ReadFrom:           protobuf.ReadFrom_Primary,
	}

	result, err := config.toProtobuf()
	if err != nil {
		t.Fatalf("Failed to convert config to protobuf: %v", err)
	}

	assert.Equal(t, expected, result)
}

func TestDefaultClusterConfig(t *testing.T) {
	config := NewGlideClusterClientConfiguration()
	expected := &protobuf.ConnectionRequest{
		TlsMode:            protobuf.TlsMode_NoTls,
		ClusterModeEnabled: true,
		ReadFrom:           protobuf.ReadFrom_Primary,
	}

	result, err := config.toProtobuf()
	if err != nil {
		t.Fatalf("Failed to convert config to protobuf: %v", err)
	}

	assert.Equal(t, expected, result)
}

func TestConfig_allFieldsSet(t *testing.T) {
	hosts := []string{"host1", "host2"}
	ports := []int{1234, 5678}
	username := "username"
	password := "password"
	timeout := 3 * time.Second
	clientName := "client name"
	retries, factor, base := 5, 10, 50
	databaseId := 1

	config := NewGlideClientConfiguration().
		WithUseTLS(true).
		WithReadFrom(PreferReplica).
		WithCredentials(NewServerCredentials(username, password)).
		WithRequestTimeout(timeout).
		WithClientName(clientName).
		WithReconnectStrategy(NewBackoffStrategy(retries, factor, base)).
		WithDatabaseId(databaseId)

	expected := &protobuf.ConnectionRequest{
		TlsMode:            protobuf.TlsMode_SecureTls,
		ReadFrom:           protobuf.ReadFrom_PreferReplica,
		ClusterModeEnabled: false,
		AuthenticationInfo: &protobuf.AuthenticationInfo{Username: username, Password: password},
		RequestTimeout:     uint32(timeout),
		ClientName:         clientName,
		ConnectionRetryStrategy: &protobuf.ConnectionRetryStrategy{
			NumberOfRetries: uint32(retries),
			Factor:          uint32(factor),
			ExponentBase:    uint32(base),
		},
		DatabaseId: uint32(databaseId),
	}

	assert.Equal(t, len(hosts), len(ports))
	for i := 0; i < len(hosts); i++ {
		config.WithAddress(&NodeAddress{hosts[i], ports[i]})
		expected.Addresses = append(
			expected.Addresses,
			&protobuf.NodeAddress{Host: hosts[i], Port: uint32(ports[i])},
		)
	}

	result, err := config.toProtobuf()
	if err != nil {
		t.Fatalf("Failed to convert config to protobuf: %v", err)
	}

	assert.Equal(t, expected, result)
}

func TestNodeAddress(t *testing.T) {
	parameters := []struct {
		input    NodeAddress
		expected *protobuf.NodeAddress
	}{
		{NodeAddress{}, &protobuf.NodeAddress{Host: DefaultHost, Port: DefaultPort}},
		{NodeAddress{Host: "host"}, &protobuf.NodeAddress{Host: "host", Port: DefaultPort}},
		{NodeAddress{Port: 1234}, &protobuf.NodeAddress{Host: DefaultHost, Port: 1234}},
		{NodeAddress{"host", 1234}, &protobuf.NodeAddress{Host: "host", Port: 1234}},
	}

	for i, parameter := range parameters {
		t.Run(fmt.Sprintf("Testing [%v]", i), func(t *testing.T) {
			result := parameter.input.toProtobuf()

			assert.Equal(t, parameter.expected, result)
		})
	}
}

func TestServerCredentials(t *testing.T) {
	parameters := []struct {
		input    *ServerCredentials
		expected *protobuf.AuthenticationInfo
	}{
		{
			NewServerCredentials("username", "password"),
			&protobuf.AuthenticationInfo{Username: "username", Password: "password"},
		},
		{
			NewServerCredentialsWithDefaultUsername("password"),
			&protobuf.AuthenticationInfo{Password: "password"},
		},
	}

	for i, parameter := range parameters {
		t.Run(fmt.Sprintf("Testing [%v]", i), func(t *testing.T) {
			result := parameter.input.toProtobuf()

			assert.Equal(t, parameter.expected, result)
		})
	}
}

func TestConfig_AzAffinity(t *testing.T) {
	hosts := []string{"host1", "host2"}
	ports := []int{1234, 5678}
	clientName := "client name"
	az := "us-east-1a"

	config := NewGlideClientConfiguration().
		WithUseTLS(true).
		WithReadFrom(AzAffinity).
		WithClientName(clientName).
		WithClientAZ(az)

	expected := &protobuf.ConnectionRequest{
		TlsMode:            protobuf.TlsMode_SecureTls,
		ReadFrom:           protobuf.ReadFrom_AZAffinity,
		ClusterModeEnabled: false,
		ClientName:         clientName,
		ClientAz:           az,
	}

	assert.Equal(t, len(hosts), len(ports))
	for i := 0; i < len(hosts); i++ {
		config.WithAddress(&NodeAddress{hosts[i], ports[i]})
		expected.Addresses = append(
			expected.Addresses,
			&protobuf.NodeAddress{Host: hosts[i], Port: uint32(ports[i])},
		)
	}

	result, err := config.toProtobuf()
	if err != nil {
		t.Fatalf("Failed to convert config to protobuf: %v", err)
	}

	assert.Equal(t, expected, result)
}
