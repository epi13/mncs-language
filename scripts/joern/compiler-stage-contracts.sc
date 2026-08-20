import io.shiftleft.semanticcpg.language.*

@main def compilerStageContracts(cpgFile: String): Unit = {
  importCpg(cpgFile)

  println("MNCS_LANGUAGE_COMPILER_STAGE_CONTRACTS")
  val targets = List(
    "request_for_program",
    "compile",
    "run_study",
    "reference_pipeline",
    "validate_integrity",
    "identity_is_valid"
  )
  targets.foreach { name =>
    val methods = cpg.method.nameExact(name)
      .filter(_.file.name.headOption.exists(path =>
        path.endsWith("mncs-compiler/src/lib.rs") || path.endsWith("mncs-model/src/compiler.rs")
      )).l
    val files = methods.map(_.filename).distinct.sorted.mkString(",")
    val callers = methods.flatMap(_.callIn.method.name.l).distinct.sorted.mkString(",")
    val callees = methods.flatMap(_.callOut.name.l).distinct.sorted.mkString(",")
    val controls = methods.flatMap(_.controlStructure.controlStructureType.l)
      .groupBy(identity).view.mapValues(_.size).toMap.toSeq.sortBy(_._1).mkString(",")
    println(s"METHOD|$name|count=${methods.size}|files=$files|callers=$callers|callees=$callees|controls=$controls")
  }

  cpg.call
    .filter(_.file.name.headOption.exists(_.endsWith("mncs-compiler/src/lib.rs")))
    .filter(call => call.name.matches("validate|semantic_graph|semantic_identities|lower_to_ir|lower_to_ssa|validate_integrity|seal"))
    .map(call => call.method.name + ":" + call.name + ":" + call.lineNumber.getOrElse(-1))
    .l.sorted.foreach(value => println(s"STAGE_CALL|$value"))
}
