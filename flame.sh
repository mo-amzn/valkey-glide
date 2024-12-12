#!/bin/bash
OUTPUT_FILE=perf.svg

if [ ! -z "$1" ]; then
    OUTPUT_FILE=$1
fi

if [ ! -d "$HOME/FlameGraph" ]; then
    cd $HOME
    git clone https://github.com/brendangregg/FlameGraph
    cd -
fi

FLAME_GRAPH_HOME=${HOME}/FlameGraph
PERF=$(which perf 2> /dev/null)
if [ -z "$PERF" ]; then
    echo "ERROR: unable to find perf command line"
    exit 1
fi

# Make sure your code is run like this:
# perf record -g --call-graph dwarf <command>
if [ ! -f "perf.data" ]; then
    echo "ERROR: unable to find perf.data file"
    exit 1
fi

${PERF} script | ${FLAME_GRAPH_HOME}/stackcollapse-perf.pl > out.perf-folded
${FLAME_GRAPH_HOME}/flamegraph.pl out.perf-folded > ${OUTPUT_FILE}
echo "SVG file:" $(readlink -f $OUTPUT_FILE)
